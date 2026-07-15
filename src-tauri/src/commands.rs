use crate::db::{get_db, next_receipt_number};
use crate::models::*;
use crate::sync::{enqueue_student_sync, sync_to_supabase};
use rusqlite::{params, OptionalExtension};
use serde_json::json;
use chrono::Datelike;

#[tauri::command]
pub fn login(username: String, password_hash: String) -> Result<User, String> {
    let db = get_db().lock().unwrap();
    let row = db
        .query_row(
            "SELECT id, username, role FROM users WHERE username=? AND password_hash=?",
            params![username, password_hash],
            |r| {
                Ok(User {
                    id: r.get(0)?,
                    username: r.get(1)?,
                    role: r.get(2)?,
                })
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;

    row.ok_or_else(|| "Invalid username or password".to_string())
}

#[tauri::command]
pub fn get_school_settings() -> Result<SchoolSettings, String> {
    let db = get_db().lock().unwrap();
    db.query_row(
        "SELECT id, school_name, logo_path, address, phone, receipt_footer, printer_name FROM school_settings WHERE id = 1",
        [],
        |r| {
            Ok(SchoolSettings {
                id: r.get(0)?,
                school_name: r.get(1)?,
                logo_path: r.get(2)?,
                address: r.get(3)?,
                phone: r.get(4)?,
                receipt_footer: r.get(5)?,
                printer_name: r.get(6)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_school_settings(settings: SchoolSettings) -> Result<(), String> {
    let db = get_db().lock().unwrap();
    db.execute(
        "UPDATE school_settings SET school_name=?, logo_path=?, address=?, phone=?, receipt_footer=?, printer_name=?, updated_at=datetime('now') WHERE id=1",
        params![
            settings.school_name,
            settings.logo_path,
            settings.address,
            settings.phone,
            settings.receipt_footer,
            settings.printer_name
        ],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_academic_years() -> Result<Vec<AcademicYear>, String> {
    let db = get_db().lock().unwrap();
    let mut stmt = db
        .prepare("SELECT id, name, start_date, end_date, is_active FROM academic_years ORDER BY id DESC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(AcademicYear {
                id: r.get(0)?,
                name: r.get(1)?,
                start_date: r.get(2)?,
                end_date: r.get(3)?,
                is_active: r.get::<_, i32>(4)? == 1,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut list = Vec::new();
    for r in rows {
        if let Ok(y) = r {
            list.push(y);
        }
    }
    Ok(list)
}

#[tauri::command]
pub fn create_academic_year(name: String, start_date: String) -> Result<AcademicYear, String> {
    let db = get_db().lock().unwrap();
    // Deactivate all existing years — new year becomes the active one
    db.execute("UPDATE academic_years SET is_active=0", [])
        .map_err(|e| e.to_string())?;
    db.execute(
        "INSERT INTO academic_years (name, start_date, is_active) VALUES (?, ?, 1)",
        params![name, start_date],
    )
    .map_err(|e| e.to_string())?;

    let id = db.last_insert_rowid();
    Ok(AcademicYear {
        id,
        name,
        start_date,
        end_date: None,
        is_active: true,
    })
}

#[tauri::command]
pub fn set_active_academic_year(id: i64) -> Result<(), String> {
    let db = get_db().lock().unwrap();
    db.execute("UPDATE academic_years SET is_active=0", [])
        .map_err(|e| e.to_string())?;
    db.execute(
        "UPDATE academic_years SET is_active=1 WHERE id=?",
        params![id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_classes(academic_year_id: Option<i64>) -> Result<Vec<Class>, String> {
    let db = get_db().lock().unwrap();
    let query = if let Some(ay_id) = academic_year_id {
        format!("SELECT id, name, section, tuition_fee, admission_fee, exam_fee, book_fee, uniform_fee,
                        lab_fee, computer_fee, sports_fee, activity_fee, maintenance_fee, academic_year_id 
                 FROM classes WHERE academic_year_id = {}", ay_id)
    } else {
        "SELECT id, name, section, tuition_fee, admission_fee, exam_fee, book_fee, uniform_fee,
                lab_fee, computer_fee, sports_fee, activity_fee, maintenance_fee, academic_year_id 
         FROM classes".to_string()
    };

    let mut stmt = db.prepare(&query).map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |r| {
            Ok(Class {
                id: r.get(0)?,
                name: r.get(1)?,
                section: r.get(2)?,
                tuition_fee: r.get(3)?,
                admission_fee: r.get(4)?,
                exam_fee: r.get(5)?,
                book_fee: r.get(6)?,
                uniform_fee: r.get(7)?,
                lab_fee: r.get(8)?,
                computer_fee: r.get(9)?,
                sports_fee: r.get(10)?,
                activity_fee: r.get(11)?,
                maintenance_fee: r.get(12)?,
                academic_year_id: r.get(13)?,
                custom_fees: Vec::new(),
            })
        })
        .map_err(|e| e.to_string())?;

    let mut list = Vec::new();
    for r in rows {
        if let Ok(mut c) = r {
            let mut sub_stmt = db
                .prepare("SELECT id, class_id, name, amount FROM fee_components WHERE class_id = ?")
                .map_err(|e| e.to_string())?;
            let custom_rows = sub_stmt
                .query_map(params![c.id], |sr| {
                    Ok(FeeComponent {
                        id: sr.get(0)?,
                        class_id: sr.get(1)?,
                        name: sr.get(2)?,
                        amount: sr.get(3)?,
                    })
                })
                .map_err(|e| e.to_string())?;
            for cr in custom_rows {
                if let Ok(fc) = cr {
                    c.custom_fees.push(fc);
                }
            }
            list.push(c);
        }
    }
    Ok(list)
}

#[tauri::command]
pub fn create_class(
    name: String,
    section: Option<String>,
    tuition_fee: f64,
    admission_fee: f64,
    exam_fee: f64,
    book_fee: f64,
    uniform_fee: f64,
    lab_fee: f64,
    computer_fee: f64,
    sports_fee: f64,
    activity_fee: f64,
    maintenance_fee: f64,
    academic_year_id: Option<i64>,
    custom_fees: Vec<ExtraFeeItem>,
) -> Result<i64, String> {
    let mut db = get_db().lock().unwrap();
    let tx = db.transaction().map_err(|e| e.to_string())?;

    tx.execute(
        "INSERT INTO classes (name, section, tuition_fee, admission_fee, exam_fee, book_fee, uniform_fee,
                             lab_fee, computer_fee, sports_fee, activity_fee, maintenance_fee, academic_year_id)
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)",
        params![
            name,
            section,
            tuition_fee,
            admission_fee,
            exam_fee,
            book_fee,
            uniform_fee,
            lab_fee,
            computer_fee,
            sports_fee,
            activity_fee,
            maintenance_fee,
            academic_year_id
        ],
    )
    .map_err(|e| e.to_string())?;

    let class_id = tx.last_insert_rowid();

    for cf in custom_fees {
        tx.execute(
            "INSERT INTO fee_components (class_id, name, amount) VALUES (?,?,?)",
            params![class_id, cf.name, cf.amount],
        )
        .map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(class_id)
}

#[tauri::command]
pub fn update_class(
    id: i64,
    name: String,
    section: Option<String>,
    tuition_fee: f64,
    admission_fee: f64,
    exam_fee: f64,
    book_fee: f64,
    uniform_fee: f64,
    lab_fee: f64,
    computer_fee: f64,
    sports_fee: f64,
    activity_fee: f64,
    maintenance_fee: f64,
    custom_fees: Vec<ExtraFeeItem>,
) -> Result<(), String> {
    let mut db = get_db().lock().unwrap();
    let tx = db.transaction().map_err(|e| e.to_string())?;

    tx.execute(
        "UPDATE classes SET name=?, section=?, tuition_fee=?, admission_fee=?, exam_fee=?, book_fee=?,
                            uniform_fee=?, lab_fee=?, computer_fee=?, sports_fee=?, activity_fee=?, maintenance_fee=?
         WHERE id=?",
        params![
            name,
            section,
            tuition_fee,
            admission_fee,
            exam_fee,
            book_fee,
            uniform_fee,
            lab_fee,
            computer_fee,
            sports_fee,
            activity_fee,
            maintenance_fee,
            id
        ],
    )
    .map_err(|e| e.to_string())?;

    tx.execute("DELETE FROM fee_components WHERE class_id=?", params![id])
        .map_err(|e| e.to_string())?;

    for cf in custom_fees {
        tx.execute(
            "INSERT INTO fee_components (class_id, name, amount) VALUES (?,?,?)",
            params![id, cf.name, cf.amount],
        )
        .map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_class(id: i64) -> Result<(), String> {
    let db = get_db().lock().unwrap();
    db.execute("DELETE FROM classes WHERE id=?", params![id])
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_students(academic_year_id: Option<i64>) -> Result<Vec<Student>, String> {
    use chrono::{Datelike, Local};
    let now = Local::now();
    let curr_year = now.year() as i32;
    let curr_month = now.month() as i32;

    let db = get_db().lock().unwrap();

    let mut query = "SELECT s.id, s.student_id, s.admission_number, s.roll_number, s.student_name, s.parent_name,
                    s.phone, s.class_id, c.name, s.bus_stop_id, b.name, s.status, s.academic_year_id,
                    s.student_type,
                    (
                      COALESCE((SELECT balance FROM bills WHERE student_id = s.student_id ORDER BY id DESC LIMIT 1), 0.0)
                    ) AS pending_balance,
                    (
                      COALESCE((SELECT SUM(balance) FROM monthly_tuition WHERE student_id = s.student_id AND (year < ?1 OR (year = ?1 AND month < ?2))), 0.0) +
                      COALESCE((SELECT SUM(balance) FROM monthly_bus_usage WHERE student_id = s.student_id AND (year < ?1 OR (year = ?1 AND month < ?2))), 0.0)
                    ) AS pending_past,
                    (
                      COALESCE((SELECT SUM(balance) FROM monthly_tuition WHERE student_id = s.student_id AND year = ?1 AND month = ?2), 0.0) +
                      COALESCE((SELECT SUM(balance) FROM monthly_bus_usage WHERE student_id = s.student_id AND year = ?1 AND month = ?2), 0.0)
                    ) AS pending_current
             FROM students s
             LEFT JOIN classes c ON s.class_id = c.id
             LEFT JOIN bus_stops b ON s.bus_stop_id = b.id".to_string();

    if let Some(ay_id) = academic_year_id {
        query.push_str(&format!(" WHERE s.academic_year_id = {}", ay_id));
    }
    query.push_str(" ORDER BY s.id DESC");

    let mut stmt = db.prepare(&query).map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![curr_year, curr_month], |r| {
            Ok(Student {
                id: r.get(0)?,
                student_id: r.get(1)?,
                admission_number: r.get(2)?,
                roll_number: r.get(3)?,
                student_name: r.get(4)?,
                parent_name: r.get(5)?,
                phone: r.get(6)?,
                class_id: r.get(7)?,
                class_name: r.get(8)?,
                bus_stop_id: r.get(9)?,
                bus_stop_name: r.get(10)?,
                status: r.get(11)?,
                academic_year_id: r.get(12)?,
                student_type: r.get(13)?,
                pending_balance: Some(r.get(14)?),
                pending_past: Some(r.get(15)?),
                pending_current: Some(r.get(16)?),
            })
        })
        .map_err(|e| e.to_string())?;

    let mut list = Vec::new();
    for r in rows {
        if let Ok(s) = r {
            list.push(s);
        }
    }
    Ok(list)
}

#[tauri::command]
pub fn get_student(id: i64) -> Result<Student, String> {
    let db = get_db().lock().unwrap();
    db.query_row(
        "SELECT s.id, s.student_id, s.admission_number, s.roll_number, s.student_name, s.parent_name,
                s.phone, s.class_id, c.name, s.bus_stop_id, b.name, s.status, s.academic_year_id,
                s.student_type,
                (
                  COALESCE((SELECT balance FROM bills WHERE student_id = s.student_id ORDER BY id DESC LIMIT 1), 0.0)
                ) AS pending_balance
         FROM students s
         LEFT JOIN classes c ON s.class_id = c.id
         LEFT JOIN bus_stops b ON s.bus_stop_id = b.id
         WHERE s.id=?",
        params![id],
        |r| {
            Ok(Student {
                id: r.get(0)?,
                student_id: r.get(1)?,
                admission_number: r.get(2)?,
                roll_number: r.get(3)?,
                student_name: r.get(4)?,
                parent_name: r.get(5)?,
                phone: r.get(6)?,
                class_id: r.get(7)?,
                class_name: r.get(8)?,
                bus_stop_id: r.get(9)?,
                bus_stop_name: r.get(10)?,
                status: r.get(11)?,
                academic_year_id: r.get(12)?,
                student_type: r.get(13)?,
                pending_balance: Some(r.get(14)?),
                pending_past: None,
                pending_current: None,
            })
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_student(
    _admission_number: String,
    _roll_number: Option<String>,
    _student_name: String,
    _parent_name: Option<String>,
    _phone: Option<String>,
    _class_id: Option<i64>,
    _bus_stop_id: Option<i64>,
    _academic_year_id: Option<i64>,
) -> Result<String, String> {
    Err("Adding students is only allowed from the Web Dashboard. Please sync data instead.".into())
}


#[tauri::command]
pub fn update_student(
    id: i64,
    roll_number: Option<String>,
    student_name: String,
    parent_name: Option<String>,
    phone: Option<String>,
    class_id: Option<i64>,
    bus_stop_id: Option<i64>,
    status: String,
    student_type: Option<String>,
) -> Result<(), String> {
    let db = get_db().lock().unwrap();
    db.execute(
        "UPDATE students SET roll_number=?, student_name=?, parent_name=?, phone=?, class_id=?, bus_stop_id=?, status=?, student_type=?, updated_at=datetime('now')
         WHERE id=?",
        params![
            roll_number,
            student_name,
            parent_name,
            phone,
            class_id,
            bus_stop_id,
            status,
            student_type,
            id
        ],
    )
    .map_err(|e| e.to_string())?;

    let student_id: String = db
        .query_row("SELECT student_id FROM students WHERE id=?", params![id], |r| {
            r.get(0)
        })
        .unwrap_or_default();

    drop(db);
    let _ = enqueue_student_sync(&student_id);
    Ok(())
}

#[tauri::command]
pub fn get_bus_stops() -> Result<Vec<BusStop>, String> {
    let db = get_db().lock().unwrap();
    let mut stmt = db
        .prepare("SELECT id, name, monthly_charge FROM bus_stops ORDER BY name ASC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(BusStop {
                id: r.get(0)?,
                name: r.get(1)?,
                monthly_charge: r.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut list = Vec::new();
    for r in rows {
        if let Ok(b) = r {
            list.push(b);
        }
    }
    Ok(list)
}

#[tauri::command]
pub fn create_bus_stop(name: String, monthly_charge: f64) -> Result<i64, String> {
    let db = get_db().lock().unwrap();
    db.execute(
        "INSERT INTO bus_stops (name, monthly_charge) VALUES (?,?)",
        params![name, monthly_charge],
    )
    .map_err(|e| e.to_string())?;
    Ok(db.last_insert_rowid())
}

#[tauri::command]
pub fn update_bus_stop(id: i64, name: String, monthly_charge: f64) -> Result<(), String> {
    let db = get_db().lock().unwrap();
    db.execute(
        "UPDATE bus_stops SET name=?, monthly_charge=? WHERE id=?",
        params![name, monthly_charge, id],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_bus_stop(id: i64) -> Result<(), String> {
    let db = get_db().lock().unwrap();
    db.execute("DELETE FROM bus_stops WHERE id=?", params![id])
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_dashboard_stats() -> Result<DashboardStats, String> {
    let db = get_db().lock().unwrap();

    let todays_collection: f64 = db
        .query_row(
            "SELECT COALESCE(SUM(amount), 0.0) FROM payments WHERE payment_date = date('now')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0.0);

    let monthly_collection: f64 = db
        .query_row(
            "SELECT COALESCE(SUM(amount), 0.0) FROM payments WHERE strftime('%Y-%m', payment_date) = strftime('%Y-%m', 'now')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0.0);

    let pending_fees: f64 = db
        .query_row("SELECT COALESCE(SUM(balance), 0.0) FROM bills", [], |r| {
            r.get(0)
        })
        .unwrap_or(0.0);

    let pending_sync: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM sync_queue WHERE status IN ('pending', 'failed')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let last_sync_time: String = db
        .query_row(
            "SELECT value FROM settings WHERE key = 'last_sync'",
            [],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "".to_string());

    let active_academic_year: String = db
        .query_row(
            "SELECT name FROM academic_years WHERE is_active=1",
            [],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "None".to_string());

    let total_students: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM students WHERE status='active'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    Ok(DashboardStats {
        todays_collection,
        monthly_collection,
        pending_fees,
        pending_sync,
        last_sync_time,
        active_academic_year,
        total_students,
    })
}

#[tauri::command]
pub fn generate_bill(req: GenerateBillRequest) -> Result<(), String> {
    let mut db = get_db().lock().unwrap();
    let tx = db.transaction().map_err(|e| e.to_string())?;

    tx.execute(
        "INSERT INTO bills (
            student_id, academic_year_id, tuition_fee, admission_fee, exam_fee, book_fee, uniform_fee,
            lab_fee, computer_fee, sports_fee, activity_fee, maintenance_fee, bus_fee, previous_balance,
            extra_fees, discount, scholarship, total_fee, amount_paid, balance, payment_status, updated_at
        ) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?, 0, ?, 'pending', datetime('now'))
        ON CONFLICT(student_id, academic_year_id) DO UPDATE SET
            tuition_fee=excluded.tuition_fee, admission_fee=excluded.admission_fee,
            exam_fee=excluded.exam_fee, book_fee=excluded.book_fee,
            uniform_fee=excluded.uniform_fee, lab_fee=excluded.lab_fee,
            computer_fee=excluded.computer_fee, sports_fee=excluded.sports_fee,
            activity_fee=excluded.activity_fee, maintenance_fee=excluded.maintenance_fee,
            bus_fee=excluded.bus_fee, previous_balance=excluded.previous_balance,
            extra_fees=excluded.extra_fees, discount=excluded.discount,
            scholarship=excluded.scholarship, total_fee=excluded.total_fee,
            balance=MAX(0.0, excluded.total_fee - amount_paid),
            payment_status=CASE WHEN MAX(0.0, excluded.total_fee - amount_paid) <= 0.001 THEN 'paid'
                                WHEN amount_paid > 0 THEN 'partial'
                                ELSE 'pending' END,
            updated_at=datetime('now')",
        params![
            req.student_id,
            req.academic_year_id,
            req.tuition_fee,
            req.admission_fee,
            req.exam_fee,
            req.book_fee,
            req.uniform_fee,
            req.lab_fee,
            req.computer_fee,
            req.sports_fee,
            req.activity_fee,
            req.maintenance_fee,
            req.bus_fee,
            req.previous_balance,
            req.extra_fees,
            req.discount,
            req.scholarship,
            req.total_fee,
            req.total_fee
        ],
    )
    .map_err(|e| e.to_string())?;

    // last_insert_rowid() returns 0 on an upsert DO UPDATE; fetch the real bill id.
    let bill_id: i64 = tx
        .query_row(
            "SELECT id FROM bills WHERE student_id=? AND academic_year_id=?",
            params![req.student_id, req.academic_year_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    tx.execute("DELETE FROM bill_items WHERE bill_id=?", params![bill_id])
        .map_err(|e| e.to_string())?;

    for item in req.extra_fee_items {
        tx.execute(
            "INSERT INTO bill_items (bill_id, name, amount) VALUES (?,?,?)",
            params![bill_id, item.name, item.amount],
        )
        .map_err(|e| e.to_string())?;
    }

    // Generate monthly tuition installments automatically
    crate::monthly::generate_monthly_tuition_for_student(
        &tx,
        &req.student_id,
        req.academic_year_id,
        req.tuition_fee,
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    drop(db);
    let _ = enqueue_student_sync(&req.student_id);
    Ok(())
}

#[tauri::command]
pub fn get_bill(student_id: String, academic_year_id: i64) -> Result<Option<Bill>, String> {
    let db = get_db().lock().unwrap();
    db.query_row(
        "SELECT id, student_id, academic_year_id, tuition_fee, admission_fee, exam_fee, book_fee, uniform_fee,
                lab_fee, computer_fee, sports_fee, activity_fee, maintenance_fee, bus_fee, previous_balance,
                extra_fees, discount, scholarship, total_fee, amount_paid, balance, payment_status, last_payment_date
         FROM bills WHERE student_id=? AND academic_year_id=?",
        params![student_id, academic_year_id],
        |r| {
            Ok(Bill {
                id: r.get(0)?,
                student_id: r.get(1)?,
                student_name: None,
                academic_year_id: r.get(2)?,
                tuition_fee: r.get(3)?,
                admission_fee: r.get(4)?,
                exam_fee: r.get(5)?,
                book_fee: r.get(6)?,
                uniform_fee: r.get(7)?,
                lab_fee: r.get(8)?,
                computer_fee: r.get(9)?,
                sports_fee: r.get(10)?,
                activity_fee: r.get(11)?,
                maintenance_fee: r.get(12)?,
                bus_fee: r.get(13)?,
                previous_balance: r.get(14)?,
                extra_fees: r.get(15)?,
                discount: r.get(16)?,
                scholarship: r.get(17)?,
                total_fee: r.get(18)?,
                amount_paid: r.get(19)?,
                balance: r.get(20)?,
                payment_status: r.get(21)?,
                last_payment_date: r.get(22)?,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn record_payment(req: RecordPaymentRequest) -> Result<String, String> {
    let mut db = get_db().lock().unwrap();
    let receipt_no = next_receipt_number(&db).map_err(|e| e.to_string())?;

    let tx = db.transaction().map_err(|e| e.to_string())?;

    let alloc_admission = req.allocated_admission.unwrap_or(0.0);
    let alloc_other     = req.allocated_other.unwrap_or(0.0);
    let alloc_tuition   = req.allocated_tuition.unwrap_or(0.0);
    let alloc_bus       = req.allocated_bus.unwrap_or(0.0);

    // Update yearly bill balance — bill is the single source of truth for total fee
    let (mut current_paid, total_fee, bill_ay_id): (f64, f64, i64) = tx
        .query_row(
            "SELECT amount_paid, total_fee, academic_year_id FROM bills WHERE id=?",
            params![req.bill_id],
            |r| Ok((r.get(0).unwrap_or(0.0), r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| e.to_string())?;

    // Bill is the single source of truth — record full payment amount against it
    let effective_bill_payment = alloc_admission + alloc_other + alloc_tuition + alloc_bus;
    let effective_bill_payment = if effective_bill_payment > 0.0 {
        effective_bill_payment
    } else {
        req.amount
    };

    current_paid += effective_bill_payment;
    let new_balance = (total_fee - current_paid).max(0.0);
    let new_status = if new_balance <= 0.0 {
        "paid"
    } else if current_paid > 0.0 {
        "partial"
    } else {
        "pending"
    };

    tx.execute(
        "UPDATE bills SET amount_paid=?, balance=?, payment_status=?, last_payment_date=?, updated_at=datetime('now') WHERE id=?",
        params![current_paid, new_balance, new_status, req.payment_date, req.bill_id],
    )
    .map_err(|e| e.to_string())?;

    // Apply tuition/bus allocations to monthly tables
    if alloc_tuition > 0.0 {
        crate::monthly::apply_monthly_tuition_payment(
            &tx, &req.student_id, bill_ay_id, alloc_tuition,
        )
        .map_err(|e| e.to_string())?;
    }
    if alloc_bus > 0.0 {
        crate::monthly::apply_monthly_bus_payment(
            &tx, &req.student_id, bill_ay_id, alloc_bus,
        )
        .map_err(|e| e.to_string())?;
    }

    tx.execute(
        "INSERT INTO payments (student_id, bill_id, payment_date, amount, payment_mode, receipt_number, academic_year_id, notes,
                               allocated_admission, allocated_other, allocated_tuition, allocated_bus)
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?)",
        params![
            req.student_id,
            req.bill_id,
            req.payment_date,
            req.amount,
            req.payment_mode,
            receipt_no,
            bill_ay_id,
            req.notes,
            alloc_admission,
            alloc_other,
            alloc_tuition,
            alloc_bus
        ],
    )
    .map_err(|e| e.to_string())?;

    let p_id = tx.last_insert_rowid();

    tx.execute(
        "INSERT INTO receipts (payment_id, receipt_number) VALUES (?,?)",
        params![p_id, receipt_no],
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    drop(db);
    let _ = enqueue_student_sync(&req.student_id);
    Ok(receipt_no)
}

#[tauri::command]
pub fn get_payments() -> Result<Vec<Payment>, String> {
    let db = get_db().lock().unwrap();
    let mut stmt = db
        .prepare(
            "SELECT p.id, p.student_id, s.student_name, p.bill_id, p.payment_date, p.amount, p.payment_mode, p.receipt_number, p.academic_year_id, p.notes,
                    p.allocated_admission, p.allocated_other, p.allocated_tuition, p.allocated_bus
             FROM payments p
             JOIN students s ON p.student_id = s.student_id
             ORDER BY p.id DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |r| {
            Ok(Payment {
                id: r.get(0)?,
                student_id: r.get(1)?,
                student_name: r.get(2)?,
                bill_id: r.get(3)?,
                payment_date: r.get(4)?,
                amount: r.get(5)?,
                payment_mode: r.get(6)?,
                receipt_number: r.get(7)?,
                academic_year_id: r.get(8)?,
                notes: r.get(9)?,
                allocated_admission: r.get(10)?,
                allocated_other: r.get(11)?,
                allocated_tuition: r.get(12)?,
                allocated_bus: r.get(13)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut list = Vec::new();
    for r in rows {
        if let Ok(p) = r {
            list.push(p);
        }
    }
    Ok(list)
}

#[tauri::command]
pub fn revert_payment(payment_id: i64) -> Result<(), String> {
    let mut db = get_db().lock().unwrap();
    let tx = db.transaction().map_err(|e| e.to_string())?;

    // 1. Get payment details
    let (bill_id, student_id, amount, notes, payment_ay_id, alloc_adm, alloc_oth, alloc_tui, alloc_bus): (
        Option<i64>,
        String,
        f64,
        Option<String>,
        Option<i64>,
        Option<f64>,
        Option<f64>,
        Option<f64>,
        Option<f64>,
    ) = tx
        .query_row(
            "SELECT bill_id, student_id, amount, notes, academic_year_id,
                    allocated_admission, allocated_other, allocated_tuition, allocated_bus
             FROM payments WHERE id = ?",
            params![payment_id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                ))
            },
        )
        .map_err(|e| format!("Payment not found: {}", e))?;

    let academic_year_id = match payment_ay_id {
        Some(ay) => ay,
        None => {
            let student_ay: Option<i64> = tx
                .query_row(
                    "SELECT academic_year_id FROM students WHERE student_id = ?",
                    params![student_id],
                    |r| r.get(0),
                )
                .optional()
                .unwrap_or(None)
                .flatten();
            match student_ay {
                Some(ay) => ay,
                None => {
                    tx.query_row("SELECT id FROM academic_years WHERE is_active=1", [], |r| r.get(0))
                        .unwrap_or(0)
                }
            }
        }
    };

    // 2. Delete from receipts associated with this payment
    tx.execute("DELETE FROM receipts WHERE payment_id = ?", params![payment_id])
        .map_err(|e| format!("Failed to delete receipts: {}", e))?;

    // 3. Delete from payments
    tx.execute("DELETE FROM payments WHERE id = ?", params![payment_id])
        .map_err(|e| format!("Failed to delete payment: {}", e))?;

    // Determine revert amounts
    let mut tuition_revert = alloc_tui.unwrap_or(0.0);
    let mut bus_revert     = alloc_bus.unwrap_or(0.0);
    let mut bill_revert_amount = alloc_adm.unwrap_or(0.0) + alloc_oth.unwrap_or(0.0)
        + alloc_tui.unwrap_or(0.0) + alloc_bus.unwrap_or(0.0);

    // If no allocation is stored, fall back to legacy parsing
    let has_any_alloc = alloc_adm.is_some() || alloc_oth.is_some() || alloc_tui.is_some() || alloc_bus.is_some();
    if !has_any_alloc {
        tuition_revert = 0.0;
        bus_revert     = 0.0;

        // Legacy payments stored real allocation in notes — parse it
        if let Some(ref n) = notes {
            if let Some(pos_t) = n.find("Tuition: ₹") {
                if let Some(pos_b) = n.find(", Bus: ₹") {
                    let start_t = pos_t + "Tuition: ₹".len();
                    if start_t <= pos_b {
                        let t_str = &n[start_t..pos_b];
                        let start_b = pos_b + ", Bus: ₹".len();
                        let mut b_str = &n[start_b..];
                        if b_str.ends_with(')') {
                            b_str = &b_str[..b_str.len() - 1];
                        }
                        if let Ok(t_val) = t_str.parse::<f64>() {
                            tuition_revert = t_val;
                        }
                        if let Ok(b_val) = b_str.parse::<f64>() {
                            bus_revert = b_val;
                        }
                    }
                }
            }
        }

        // Bill revert is the portion NOT going to tuition/bus
        bill_revert_amount = (amount - tuition_revert - bus_revert).max(0.0);
    }

    // 4. Update the bill balance
    if let Some(bill_id) = bill_id {
        let (mut current_paid, total_fee): (f64, f64) = tx
            .query_row(
                "SELECT amount_paid, total_fee FROM bills WHERE id = ?",
                params![bill_id],
                |r| Ok((r.get(0).unwrap_or(0.0), r.get(1)?)),
            )
            .map_err(|e| format!("Bill not found: {}", e))?;

        current_paid = (current_paid - bill_revert_amount).max(0.0);
        let new_balance = (total_fee - current_paid).max(0.0);
        let new_status = if new_balance <= 0.0 {
            "paid"
        } else if current_paid > 0.0 {
            "partial"
        } else {
            "pending"
        };

        // Find new last_payment_date (if other payments exist for this bill)
        let last_payment_date: Option<String> = tx
            .query_row(
                "SELECT payment_date FROM payments WHERE bill_id = ? ORDER BY id DESC LIMIT 1",
                params![bill_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .flatten();

        tx.execute(
            "UPDATE bills SET amount_paid = ?, balance = ?, payment_status = ?, last_payment_date = ?, updated_at = datetime('now') WHERE id = ?",
            params![current_paid, new_balance, new_status, last_payment_date, bill_id],
        )
        .map_err(|e| format!("Failed to update bill: {}", e))?;
    }

    // 5. Update monthly balances if any tuition/bus was reverted
    if tuition_revert > 0.0 {
        let mut stmt = tx.prepare(
            "SELECT id, amount_paid, monthly_amount FROM monthly_tuition
             WHERE student_id = ? AND academic_year_id = ? AND amount_paid > 0
             ORDER BY year DESC, month DESC",
        ).map_err(|e| e.to_string())?;
        
        let rows: Vec<(i64, f64, f64)> = stmt
            .query_map(params![student_id, academic_year_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
            
        let mut rem_tuition = tuition_revert;
        for (id, amt_paid, monthly_amt) in rows {
            if rem_tuition <= 0.0 { break; }
            let reduce = rem_tuition.min(amt_paid);
            let new_paid = (amt_paid - reduce).max(0.0);
            let new_bal = (monthly_amt - new_paid).max(0.0);
            let new_status = if new_bal <= 0.001 { "paid" } else if new_paid <= 0.001 { "unpaid" } else { "partial" };
            
            tx.execute(
                "UPDATE monthly_tuition SET amount_paid = ?, balance = ?, status = ?, updated_at = datetime('now') WHERE id = ?",
                params![new_paid, new_bal, new_status, id]
            ).map_err(|e| e.to_string())?;
            rem_tuition -= reduce;
        }
    }

    if bus_revert > 0.0 {
        let mut stmt = tx.prepare(
            "SELECT id, amount_paid, bus_fee FROM monthly_bus_usage
             WHERE student_id = ? AND academic_year_id = ? AND amount_paid > 0
             ORDER BY year DESC, month DESC",
        ).map_err(|e| e.to_string())?;
        
        let rows: Vec<(i64, f64, f64)> = stmt
            .query_map(params![student_id, academic_year_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
            
        let mut rem_bus = bus_revert;
        for (id, amt_paid, bus_fee) in rows {
            if rem_bus <= 0.0 { break; }
            let reduce = rem_bus.min(amt_paid);
            let new_paid = (amt_paid - reduce).max(0.0);
            let new_bal = (bus_fee - new_paid).max(0.0);
            let new_status = if new_bal <= 0.001 { "paid" } else if new_paid <= 0.001 { "unpaid" } else { "partial" };
            
            tx.execute(
                "UPDATE monthly_bus_usage SET amount_paid = ?, balance = ?, status = ? WHERE id = ?",
                params![new_paid, new_bal, new_status, id]
            ).map_err(|e| e.to_string())?;
            rem_bus -= reduce;
        }
    }

    tx.commit().map_err(|e| e.to_string())?;
    drop(db);
    let _ = enqueue_student_sync(&student_id);
    Ok(())
}

#[tauri::command]
pub fn get_receipt_data(receipt_number: String) -> Result<ReceiptData, String> {
    let db = get_db().lock().unwrap();
    let payment = db
        .query_row(
            "SELECT id, student_id, bill_id, payment_date, amount, payment_mode, receipt_number, academic_year_id, notes,
                    allocated_admission, allocated_other, allocated_tuition, allocated_bus
             FROM payments WHERE receipt_number = ?",
            params![receipt_number],
            |r| {
                Ok(Payment {
                    id: r.get(0)?,
                    student_id: r.get(1)?,
                    student_name: None,
                    bill_id: r.get(2)?,
                    payment_date: r.get(3)?,
                    amount: r.get(4)?,
                    payment_mode: r.get(5)?,
                    receipt_number: r.get(6)?,
                    academic_year_id: r.get(7)?,
                    notes: r.get(8)?,
                    allocated_admission: r.get(9)?,
                    allocated_other: r.get(10)?,
                    allocated_tuition: r.get(11)?,
                    allocated_bus: r.get(12)?,
                })
            },
        )
        .map_err(|e| e.to_string())?;

    let student = db
        .query_row(
            "SELECT s.id, s.student_id, s.admission_number, s.roll_number, s.student_name, s.parent_name,
                    s.phone, s.class_id, c.name, s.bus_stop_id, b.name, s.status, s.academic_year_id,
                    s.student_type
             FROM students s
             LEFT JOIN classes c ON s.class_id = c.id
             LEFT JOIN bus_stops b ON s.bus_stop_id = b.id
             WHERE s.student_id = ?",
            params![payment.student_id],
            |r| {
                Ok(Student {
                    id: r.get(0)?,
                    student_id: r.get(1)?,
                    admission_number: r.get(2)?,
                    roll_number: r.get(3)?,
                    student_name: r.get(4)?,
                    parent_name: r.get(5)?,
                    phone: r.get(6)?,
                    class_id: r.get(7)?,
                    class_name: r.get(8)?,
                    bus_stop_id: r.get(9)?,
                    bus_stop_name: r.get(10)?,
                    status: r.get(11)?,
                    academic_year_id: r.get(12)?,
                    student_type: r.get(13)?,
                    pending_balance: None,
                    pending_past: None,
                    pending_current: None,
                })
            },
        )
        .map_err(|e| e.to_string())?;

    let bill = if let Some(bill_id) = payment.bill_id {
        db.query_row(
            "SELECT id, student_id, academic_year_id, tuition_fee, admission_fee, exam_fee, book_fee, uniform_fee,
                    lab_fee, computer_fee, sports_fee, activity_fee, maintenance_fee, bus_fee, previous_balance,
                    extra_fees, discount, scholarship, total_fee, amount_paid, balance, payment_status, last_payment_date
             FROM bills WHERE id = ?",
            params![bill_id],
            |r| {
                Ok(Bill {
                    id: r.get(0)?,
                    student_id: r.get(1)?,
                    student_name: None,
                    academic_year_id: r.get(2)?,
                    tuition_fee: r.get(3)?,
                    admission_fee: r.get(4)?,
                    exam_fee: r.get(5)?,
                    book_fee: r.get(6)?,
                    uniform_fee: r.get(7)?,
                    lab_fee: r.get(8)?,
                    computer_fee: r.get(9)?,
                    sports_fee: r.get(10)?,
                    activity_fee: r.get(11)?,
                    maintenance_fee: r.get(12)?,
                    bus_fee: r.get(13)?,
                    previous_balance: r.get(14)?,
                    extra_fees: r.get(15)?,
                    discount: r.get(16)?,
                    scholarship: r.get(17)?,
                    total_fee: r.get(18)?,
                    amount_paid: r.get(19)?,
                    balance: r.get(20)?,
                    payment_status: r.get(21)?,
                    last_payment_date: r.get(22)?,
                })
            },
        )
        .map_err(|e| e.to_string())?
    } else {
        // Parse tuition vs bus split from notes safely without hardcoded UTF-8 offsets
        let mut tuition_fee = payment.amount;
        let mut bus_fee = 0.0;
        if let Some(ref notes) = payment.notes {
            if let Some(pos_t) = notes.find("Tuition: ₹") {
                if let Some(pos_b) = notes.find(", Bus: ₹") {
                    let start_t = pos_t + "Tuition: ₹".len();
                    if start_t <= pos_b {
                        let t_val_str = &notes[start_t..pos_b];
                        let start_b = pos_b + ", Bus: ₹".len();
                        let mut b_val_str = &notes[start_b..];
                        if b_val_str.ends_with(')') {
                            b_val_str = &b_val_str[..b_val_str.len() - 1];
                        }
                        if let Ok(t_val) = t_val_str.parse::<f64>() {
                            tuition_fee = t_val;
                        }
                        if let Ok(b_val) = b_val_str.parse::<f64>() {
                            bus_fee = b_val;
                        }
                    }
                }
            }
        }
        Bill {
            id: 0,
            student_id: payment.student_id.clone(),
            student_name: None,
            academic_year_id: payment.academic_year_id.or(student.academic_year_id).unwrap_or(0),
            tuition_fee,
            admission_fee: 0.0,
            exam_fee: 0.0,
            book_fee: 0.0,
            uniform_fee: 0.0,
            lab_fee: 0.0,
            computer_fee: 0.0,
            sports_fee: 0.0,
            activity_fee: 0.0,
            maintenance_fee: 0.0,
            bus_fee,
            previous_balance: 0.0,
            extra_fees: 0.0,
            discount: 0.0,
            scholarship: 0.0,
            total_fee: payment.amount,
            amount_paid: payment.amount,
            balance: 0.0,
            payment_status: "paid".to_string(),
            last_payment_date: Some(payment.payment_date.clone()),
        }
    };

    let school = db
        .query_row(
            "SELECT id, school_name, logo_path, address, phone, receipt_footer, printer_name FROM school_settings WHERE id = 1",
            [],
            |r| {
                Ok(SchoolSettings {
                    id: r.get(0)?,
                    school_name: r.get(1)?,
                    logo_path: r.get(2)?,
                    address: r.get(3)?,
                    phone: r.get(4)?,
                    receipt_footer: r.get(5)?,
                    printer_name: r.get(6)?,
                })
            },
        )
        .map_err(|e| e.to_string())?;

    // Resolve academic year name: prefer payment's AY id, fall back to bill's AY id
    let ay_id_for_name = payment.academic_year_id.unwrap_or(bill.academic_year_id);
    let academic_year_name: String = db
        .query_row(
            "SELECT name FROM academic_years WHERE id = ?",
            params![ay_id_for_name],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "Unknown".to_string());

    let mut stmt = db
        .prepare("SELECT name, amount FROM bill_items WHERE bill_id = ?")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![payment.bill_id], |r| {
            Ok(ExtraFeeItem {
                name: r.get(0)?,
                amount: r.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut bill_items = Vec::new();
    for r in rows {
        if let Ok(item) = r {
            bill_items.push(item);
        }
    }

    Ok(ReceiptData {
        receipt_number,
        student,
        payment,
        bill,
        school,
        academic_year_name,
        bill_items,
    })
}

#[tauri::command]
pub fn get_bill_items(bill_id: i64) -> Result<Vec<ExtraFeeItem>, String> {
    let db = get_db().lock().unwrap();
    let mut stmt = db
        .prepare("SELECT name, amount FROM bill_items WHERE bill_id = ?")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![bill_id], |r| {
            Ok(ExtraFeeItem {
                name: r.get(0)?,
                amount: r.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut list = Vec::new();
    for r in rows {
        if let Ok(item) = r {
            list.push(item);
        }
    }
    Ok(list)
}

#[tauri::command]
pub fn get_settings() -> Result<serde_json::Value, String> {
    let db = get_db().lock().unwrap();
    let mut stmt = db
        .prepare("SELECT key, value FROM settings")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?;

    let mut map = serde_json::Map::new();
    for r in rows {
        if let Ok((k, v)) = r {
            map.insert(k, serde_json::Value::String(v));
        }
    }
    Ok(serde_json::Value::Object(map))
}

#[tauri::command]
pub fn update_setting(key: String, value: String) -> Result<(), String> {
    let db = get_db().lock().unwrap();
    db.execute(
        "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?, ?, datetime('now'))",
        params![key, value],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

// ── MONTHLY TUITION & BUS COMMANDS ─────────────────────────────────────────

const MONTH_NAMES: [&str; 12] = [
    "January","February","March","April","May","June",
    "July","August","September","October","November","December",
];

/// Configure start/end months for an academic year
#[tauri::command]
pub fn configure_year_months(
    academic_year_id: i64,
    start_month: u32,
    end_month: u32,
    start_year: i32,
) -> Result<(), String> {
    let db = get_db().lock().unwrap();
    db.execute(
        "INSERT OR REPLACE INTO academic_year_months
         (academic_year_id, start_month, end_month, start_year)
         VALUES (?, ?, ?, ?)",
        params![academic_year_id, start_month, end_month, start_year],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// Get month config for an academic year
#[tauri::command]
pub fn get_year_months(academic_year_id: i64) -> Result<AcademicYearMonths, String> {
    let db = get_db().lock().unwrap();
    db.query_row(
        "SELECT academic_year_id, start_month, end_month, start_year
         FROM academic_year_months WHERE academic_year_id = ?",
        params![academic_year_id],
        |r| Ok(AcademicYearMonths {
            academic_year_id: r.get(0)?,
            start_month: r.get(1)?,
            end_month: r.get(2)?,
            start_year: r.get(3)?,
        }),
    )
    .map_err(|e| e.to_string())
}

/// Generate monthly tuition records for ALL active students (idempotent)
#[tauri::command]
pub fn generate_monthly_tuition_all(academic_year_id: i64) -> Result<u64, String> {
    let db = get_db().lock().unwrap();

    // Get all active students in this academic year
    let mut stmt = db
        .prepare(
            "SELECT s.student_id, c.tuition_fee
             FROM students s
             LEFT JOIN classes c ON s.class_id = c.id
             WHERE s.academic_year_id = ? AND s.status = 'active'",
        )
        .map_err(|e| e.to_string())?;

    let students: Vec<(String, f64)> = stmt
        .query_map(params![academic_year_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1).unwrap_or(0.0)))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let count = students.len() as u64;
    for (student_id, yearly_tuition) in students {
        crate::monthly::generate_monthly_tuition_for_student(
            &db,
            &student_id,
            academic_year_id,
            yearly_tuition,
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(count)
}

/// Generate monthly tuition for ONE student (called when bill is first opened)
#[tauri::command]
pub fn generate_monthly_tuition_for_student(
    student_id: String,
    academic_year_id: i64,
) -> Result<(), String> {
    let db = get_db().lock().unwrap();
    let yearly_tuition: f64 = db
        .query_row(
            "SELECT c.tuition_fee FROM students s
             LEFT JOIN classes c ON s.class_id = c.id
             WHERE s.student_id = ?",
            params![student_id],
            |r| r.get(0),
        )
        .unwrap_or(0.0);

    crate::monthly::generate_monthly_tuition_for_student(
        &db,
        &student_id,
        academic_year_id,
        yearly_tuition,
    )
    .map_err(|e| e.to_string())
}

/// Get per-month tuition status for a student (for dashboard / bill view)
#[tauri::command]
pub fn get_monthly_tuition_status(
    student_id: String,
    academic_year_id: i64,
) -> Result<Vec<MonthlyTuitionItem>, String> {
    let db = get_db().lock().unwrap();
    let mut stmt = db
        .prepare(
            "SELECT id, student_id, academic_year_id, month, year,
                    monthly_amount, amount_paid, balance, status
             FROM monthly_tuition
             WHERE student_id = ? AND academic_year_id = ?
             ORDER BY year ASC, month ASC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![student_id, academic_year_id], |r| {
            let month: u32 = r.get(3)?;
            Ok(MonthlyTuitionItem {
                id: r.get(0)?,
                student_id: r.get(1)?,
                academic_year_id: r.get(2)?,
                month,
                year: r.get(4)?,
                month_name: MONTH_NAMES[(month as usize).saturating_sub(1)].to_string(),
                monthly_amount: r.get(5)?,
                amount_paid: r.get(6)?,
                balance: r.get(7)?,
                status: r.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?;

    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// Get outstanding (unpaid/partial) tuition months up to and including given month/year
#[tauri::command]
pub fn get_outstanding_tuition(
    student_id: String,
    academic_year_id: i64,
    up_to_month: u32,
    up_to_year: i32,
) -> Result<Vec<MonthlyTuitionItem>, String> {
    let db = get_db().lock().unwrap();
    let mut stmt = db
        .prepare(
            "SELECT id, student_id, academic_year_id, month, year,
                    monthly_amount, amount_paid, balance, status
             FROM monthly_tuition
             WHERE student_id = ? AND academic_year_id = ?
               AND status IN ('unpaid', 'partial')
               AND (year < ? OR (year = ? AND month <= ?))
             ORDER BY year ASC, month ASC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(
            params![student_id, academic_year_id, up_to_year, up_to_year, up_to_month],
            |r| {
                let month: u32 = r.get(3)?;
                Ok(MonthlyTuitionItem {
                    id: r.get(0)?,
                    student_id: r.get(1)?,
                    academic_year_id: r.get(2)?,
                    month,
                    year: r.get(4)?,
                    month_name: MONTH_NAMES[(month as usize).saturating_sub(1)].to_string(),
                    monthly_amount: r.get(5)?,
                    amount_paid: r.get(6)?,
                    balance: r.get(7)?,
                    status: r.get(8)?,
                })
            },
        )
        .map_err(|e| e.to_string())?;

    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// Get full monthly bill summary for a student (current + overdue)
#[tauri::command]
pub fn get_monthly_bill_summary(
    student_id: String,
    academic_year_id: i64,
) -> Result<MonthlyBillSummary, String> {
    let db = get_db().lock().unwrap();

    // Get student name
    let (student_name, _bus_stop_id): (String, Option<i64>) = db
        .query_row(
            "SELECT student_name, bus_stop_id FROM students WHERE student_id = ?",
            params![student_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    // Current month/year from system
    let now = chrono::Local::now();
    let current_month = now.month();
    let current_year = now.year();

    // Get all outstanding tuition months up to current
    let mut stmt = db
        .prepare(
            "SELECT id, student_id, academic_year_id, month, year,
                    monthly_amount, amount_paid, balance, status
             FROM monthly_tuition
             WHERE student_id = ? AND academic_year_id = ?
               AND status IN ('unpaid', 'partial')
               AND (year < ? OR (year = ? AND month <= ?))
             ORDER BY year ASC, month ASC",
        )
        .map_err(|e| e.to_string())?;

    let months_overdue: Vec<MonthlyTuitionItem> = stmt
        .query_map(
            params![student_id, academic_year_id, current_year, current_year, current_month],
            |r| {
                let m: u32 = r.get(3)?;
                Ok(MonthlyTuitionItem {
                    id: r.get(0)?,
                    student_id: r.get(1)?,
                    academic_year_id: r.get(2)?,
                    month: m,
                    year: r.get(4)?,
                    month_name: MONTH_NAMES[(m as usize).saturating_sub(1)].to_string(),
                    monthly_amount: r.get(5)?,
                    amount_paid: r.get(6)?,
                    balance: r.get(7)?,
                    status: r.get(8)?,
                })
            },
        )
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let current_month_tuition = months_overdue
        .iter()
        .find(|m| m.month == current_month && m.year == current_year)
        .map(|m| m.balance)
        .unwrap_or(0.0);

    let outstanding_tuition: f64 = months_overdue.iter().map(|m| m.balance).sum();

    // Get outstanding bus months
    let mut bus_stmt = db
        .prepare(
            "SELECT id, student_id, academic_year_id, month, year,
                    bus_used, bus_fee, amount_paid, balance, status
             FROM monthly_bus_usage
             WHERE student_id = ? AND academic_year_id = ?
               AND bus_used = 1 AND status IN ('unpaid', 'partial')
               AND (year < ? OR (year = ? AND month <= ?))
             ORDER BY year ASC, month ASC",
        )
        .map_err(|e| e.to_string())?;

    let bus_months_overdue: Vec<MonthlyBusUsageItem> = bus_stmt
        .query_map(
            params![student_id, academic_year_id, current_year, current_year, current_month],
            |r| {
                let m: u32 = r.get(3)?;
                Ok(MonthlyBusUsageItem {
                    id: r.get(0)?,
                    student_id: r.get(1)?,
                    student_name: None,
                    academic_year_id: r.get(2)?,
                    month: m,
                    year: r.get(4)?,
                    month_name: MONTH_NAMES[(m as usize).saturating_sub(1)].to_string(),
                    bus_used: r.get::<_, i32>(5)? == 1,
                    bus_fee: r.get(6)?,
                    amount_paid: r.get(7)?,
                    balance: r.get(8)?,
                    status: r.get(9)?,
                })
            },
        )
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let outstanding_bus: f64 = bus_months_overdue.iter().map(|b| b.balance).sum();
    let current_bus_fee = bus_months_overdue
        .iter()
        .find(|b| b.month == current_month && b.year == current_year)
        .map(|b| b.balance)
        .unwrap_or(0.0);

    Ok(MonthlyBillSummary {
        student_id,
        student_name,
        academic_year_id,
        current_month,
        current_year,
        current_month_tuition,
        outstanding_tuition,
        outstanding_bus,
        current_bus_fee,
        months_overdue,
        bus_months_overdue,
    })
}

/// Set/update bus usage for a student for a given month
#[tauri::command]
pub fn set_monthly_bus_usage(
    student_id: String,
    academic_year_id: i64,
    month: u32,
    year: i32,
    bus_used: bool,
) -> Result<(), String> {
    let db = get_db().lock().unwrap();

    // Get bus fee from student's bus stop
    let bus_fee: f64 = if bus_used {
        db.query_row(
            "SELECT bs.monthly_charge FROM students s
             JOIN bus_stops bs ON s.bus_stop_id = bs.id
             WHERE s.student_id = ?",
            params![student_id],
            |r| r.get(0),
        )
        .unwrap_or(0.0)
    } else {
        0.0
    };

    let status = if bus_used { "unpaid" } else { "paid" };
    let balance = if bus_used { bus_fee } else { 0.0 };

    db.execute(
        "INSERT INTO monthly_bus_usage
             (student_id, academic_year_id, month, year, bus_used, bus_fee, balance, status)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(student_id, academic_year_id, month, year)
         DO UPDATE SET bus_used=excluded.bus_used,
                       bus_fee=excluded.bus_fee,
                       balance=CASE WHEN excluded.bus_used=1 THEN MAX(0.0, excluded.bus_fee - amount_paid) ELSE 0.0 END,
                       status=CASE WHEN excluded.bus_used=0 THEN 'paid'
                                   WHEN MAX(0.0, excluded.bus_fee - amount_paid) <= 0.001 THEN 'paid'
                                   WHEN amount_paid > 0 THEN 'partial'
                                   ELSE 'unpaid' END",
        params![
            student_id,
            academic_year_id,
            month,
            year,
            bus_used as i32,
            bus_fee,
            balance,
            status
        ],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// Get bus usage for a specific student/month
#[tauri::command]
pub fn get_student_bus_usage(
    student_id: String,
    academic_year_id: i64,
) -> Result<Vec<MonthlyBusUsageItem>, String> {
    let db = get_db().lock().unwrap();
    let mut stmt = db
        .prepare(
            "SELECT id, student_id, academic_year_id, month, year,
                    bus_used, bus_fee, amount_paid, balance, status
             FROM monthly_bus_usage
             WHERE student_id = ? AND academic_year_id = ?
             ORDER BY year ASC, month ASC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![student_id, academic_year_id], |r| {
            let m: u32 = r.get(3)?;
            Ok(MonthlyBusUsageItem {
                id: r.get(0)?,
                student_id: r.get(1)?,
                student_name: None,
                academic_year_id: r.get(2)?,
                month: m,
                year: r.get(4)?,
                month_name: MONTH_NAMES[(m as usize).saturating_sub(1)].to_string(),
                bus_used: r.get::<_, i32>(5)? == 1,
                bus_fee: r.get(6)?,
                amount_paid: r.get(7)?,
                balance: r.get(8)?,
                status: r.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?;

    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// Record a monthly payment (distributed across oldest unpaid tuition + bus)
#[tauri::command]
pub fn record_monthly_payment(
    student_id: String,
    academic_year_id: i64,
    tuition_amount: f64,
    bus_amount: f64,
    allocated_admission: Option<f64>,
    allocated_other: Option<f64>,
    payment_mode: String,
    payment_date: String,
    notes: Option<String>,
) -> Result<String, String> {
    let mut db = get_db().lock().unwrap();
    let receipt_no = crate::db::next_receipt_number(&db).map_err(|e| e.to_string())?;
    let total_amount = tuition_amount + bus_amount + allocated_admission.unwrap_or(0.0) + allocated_other.unwrap_or(0.0);

    let tx = db.transaction().map_err(|e| e.to_string())?;

    // Distribute tuition payment across unpaid months oldest-first
    if tuition_amount > 0.0 {
        crate::monthly::apply_monthly_tuition_payment(
            &tx,
            &student_id,
            academic_year_id,
            tuition_amount,
        )
        .map_err(|e| e.to_string())?;
    }

    // Distribute bus payment across unpaid months oldest-first
    if bus_amount > 0.0 {
        crate::monthly::apply_monthly_bus_payment(
            &tx,
            &student_id,
            academic_year_id,
            bus_amount,
        )
        .map_err(|e| e.to_string())?;
    }

    // Record payment entry (using bill_id = NULL for monthly payments not tied to yearly bill)
    let payment_notes = match &notes {
        Some(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => format!("Tuition: ₹{:.2}, Bus: ₹{:.2}", tuition_amount, bus_amount),
    };

    tx.execute(
        "INSERT INTO payments
             (student_id, bill_id, payment_date, amount, payment_mode, receipt_number, academic_year_id, notes,
              allocated_admission, allocated_other, allocated_tuition, allocated_bus)
         VALUES (?, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            student_id,
            payment_date,
            total_amount,
            payment_mode,
            receipt_no,
            academic_year_id,
            payment_notes,
            allocated_admission.unwrap_or(0.0),
            allocated_other.unwrap_or(0.0),
            tuition_amount,
            bus_amount
        ],
    )
    .map_err(|e| e.to_string())?;

    let p_id = tx.last_insert_rowid();
    tx.execute(
        "INSERT INTO receipts (payment_id, receipt_number) VALUES (?,?)",
        params![p_id, receipt_no],
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    drop(db);
    let _ = crate::sync::enqueue_student_sync(&student_id);
    Ok(receipt_no)
}

/// Get bus fee report for an academic year (optionally filtered by month)
#[tauri::command]
pub fn get_bus_report(
    academic_year_id: i64,
    month: Option<u32>,
) -> Result<Vec<BusReportItem>, String> {
    let db = get_db().lock().unwrap();

    let sql = if month.is_some() {
        "SELECT mbu.student_id, s.student_name, mbu.month, mbu.year,
                mbu.bus_used, mbu.bus_fee, mbu.amount_paid, mbu.balance, mbu.status
         FROM monthly_bus_usage mbu
         JOIN students s ON mbu.student_id = s.student_id
         WHERE mbu.academic_year_id = ? AND mbu.month = ?
         ORDER BY s.student_name ASC, mbu.year ASC, mbu.month ASC"
    } else {
        "SELECT mbu.student_id, s.student_name, mbu.month, mbu.year,
                mbu.bus_used, mbu.bus_fee, mbu.amount_paid, mbu.balance, mbu.status
         FROM monthly_bus_usage mbu
         JOIN students s ON mbu.student_id = s.student_id
         WHERE mbu.academic_year_id = ?
         ORDER BY s.student_name ASC, mbu.year ASC, mbu.month ASC"
    };

    let rows: Vec<BusReportItem> = if let Some(m) = month {
        let mut stmt = db.prepare(sql).map_err(|e| e.to_string())?;
        let mapped = stmt.query_map(params![academic_year_id, m], |r| {
            let mo: u32 = r.get(2)?;
            Ok(BusReportItem {
                student_id: r.get(0)?,
                student_name: r.get(1)?,
                month: mo,
                year: r.get(3)?,
                month_name: MONTH_NAMES[(mo as usize).saturating_sub(1)].to_string(),
                bus_used: r.get::<_, i32>(4)? == 1,
                bus_fee: r.get(5)?,
                amount_paid: r.get(6)?,
                balance: r.get(7)?,
                status: r.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?;
        let res: Vec<BusReportItem> = mapped.filter_map(|r| r.ok()).collect();
        res
    } else {
        let mut stmt = db.prepare(sql).map_err(|e| e.to_string())?;
        let mapped = stmt.query_map(params![academic_year_id], |r| {
            let mo: u32 = r.get(2)?;
            Ok(BusReportItem {
                student_id: r.get(0)?,
                student_name: r.get(1)?,
                month: mo,
                year: r.get(3)?,
                month_name: MONTH_NAMES[(mo as usize).saturating_sub(1)].to_string(),
                bus_used: r.get::<_, i32>(4)? == 1,
                bus_fee: r.get(5)?,
                amount_paid: r.get(6)?,
                balance: r.get(7)?,
                status: r.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?;
        let res: Vec<BusReportItem> = mapped.filter_map(|r| r.ok()).collect();
        res
    };

    Ok(rows)
}

#[tauri::command]
pub async fn sync_students_classes() -> Result<(), String> {
    let supabase_url = "https://pgnslzcznvtddsgmvipk.supabase.co";
    let service_key = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6InBnbnNsemN6bnZ0ZGRzZ212aXBrIiwicm9sZSI6InNlcnZpY2Vfcm9sZSIsImlhdCI6MTc4MTk0MTI5NywiZXhwIjoyMDk3NTE3Mjk3fQ.AC-vQjvYbDwHK01We0_pE56pv3lRu6rwL-U5Gzy71qc";

    if let Err(e) = crate::sync::sync_from_supabase(supabase_url, service_key).await {
        log::error!("Error pulling from Supabase: {}", e);
        return Err(format!("Pull Sync failed: {}", e));
    }
    Ok(())
}

#[tauri::command]
pub async fn sync_fees() -> Result<(), String> {
    let supabase_url = "https://pgnslzcznvtddsgmvipk.supabase.co";
    let service_key = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6InBnbnNsemN6bnZ0ZGRzZ212aXBrIiwicm9sZSI6InNlcnZpY2Vfcm9sZSIsImlhdCI6MTc4MTk0MTI5NywiZXhwIjoyMDk3NTE3Mjk3fQ.AC-vQjvYbDwHK01We0_pE56pv3lRu6rwL-U5Gzy71qc";

    sync_to_supabase(supabase_url, service_key)
        .await
        .map_err(|e| e.to_string())
}


#[tauri::command]
pub fn get_sync_queue() -> Result<Vec<SyncQueueItem>, String> {
    let db = get_db().lock().unwrap();
    let mut stmt = db
        .prepare("SELECT id, student_id, status, retry_count, last_attempt_at, created_at, error_message FROM sync_queue ORDER BY id DESC LIMIT 50")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(SyncQueueItem {
                id: r.get(0)?,
                student_id: r.get(1)?,
                status: r.get(2)?,
                retry_count: r.get(3)?,
                last_attempt_at: r.get(4)?,
                created_at: r.get(5)?,
                error_message: r.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut list = Vec::new();
    for r in rows {
        if let Ok(item) = r {
            list.push(item);
        }
    }
    Ok(list)
}

#[tauri::command]
pub fn clear_sync_logs() -> Result<(), String> {
    let db = get_db().lock().unwrap();
    db.execute(
        "DELETE FROM sync_queue WHERE status = 'success'",
        [],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn retry_failed_sync() -> Result<u64, String> {
    let db = get_db().lock().unwrap();
    // Reset failed items back to pending so sync_to_supabase picks them up again
    db.execute(
        "UPDATE sync_queue SET status='pending', error_message=NULL WHERE status='failed'",
        [],
    )
    .map(|n| n as u64)
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_reports(start_date: String, end_date: String) -> Result<serde_json::Value, String> {
    let db = get_db().lock().unwrap();

    let mut p_stmt = db
        .prepare(
            "SELECT p.receipt_number, s.student_name, c.name, p.amount, p.payment_mode, p.payment_date, p.id
             FROM payments p
             JOIN students s ON p.student_id = s.student_id
             LEFT JOIN classes c ON s.class_id = c.id
             WHERE p.payment_date >= ? AND p.payment_date <= ?
             ORDER BY p.id ASC",
        )
        .map_err(|e| e.to_string())?;

    let p_rows = p_stmt
        .query_map(params![start_date, end_date], |r| {
            Ok(json!({
                "receipt_number": r.get::<_, String>(0)?,
                "student_name": r.get::<_, String>(1)?,
                "class_name": r.get::<_, Option<String>>(2)?,
                "amount": r.get::<_, f64>(3)?,
                "payment_mode": r.get::<_, String>(4)?,
                "payment_date": r.get::<_, String>(5)?,
                "id": r.get::<_, i64>(6)?,
            }))
        })
        .map_err(|e| e.to_string())?;

    let mut payments = Vec::new();
    let mut total_collection = 0.0;
    for pr in p_rows {
        if let Ok(val) = pr {
            total_collection += val["amount"].as_f64().unwrap_or(0.0);
            payments.push(val);
        }
    }

    let mut c_stmt = db
        .prepare(
            "SELECT c.name, COUNT(s.id), COALESCE(SUM(b.total_fee), 0.0), COALESCE(SUM(b.amount_paid), 0.0), COALESCE(SUM(b.balance), 0.0)
             FROM classes c
             LEFT JOIN students s ON s.class_id = c.id
             LEFT JOIN bills b ON b.student_id = s.student_id
             GROUP BY c.id",
        )
        .map_err(|e| e.to_string())?;

    let c_rows = c_stmt
        .query_map([], |r| {
            Ok(json!({
                "class_name": r.get::<_, String>(0)?,
                "student_count": r.get::<_, i64>(1)?,
                "total_fee": r.get::<_, f64>(2)?,
                "amount_paid": r.get::<_, f64>(3)?,
                "balance": r.get::<_, f64>(4)?,
            }))
        })
        .map_err(|e| e.to_string())?;

    let mut classes = Vec::new();
    for cr in c_rows {
        if let Ok(val) = cr {
            classes.push(val);
        }
    }

    Ok(json!({
        "payments": payments,
        "total_collection": total_collection,
        "classes": classes,
    }))
}

#[tauri::command]
pub fn search_students(query: String) -> Result<Vec<Student>, String> {
    use chrono::{Datelike, Local};
    let now = Local::now();
    let curr_year = now.year() as i32;
    let curr_month = now.month() as i32;

    let db = get_db().lock().unwrap();
    let search_pattern = format!("%{}%", query);

    let mut stmt = db
        .prepare(
            "SELECT s.id, s.student_id, s.admission_number, s.roll_number, s.student_name, s.parent_name,
                    s.phone, s.class_id, c.name, s.bus_stop_id, b.name, s.status, s.academic_year_id,
                    s.student_type,
                    (
                      COALESCE((SELECT balance FROM bills WHERE student_id = s.student_id ORDER BY id DESC LIMIT 1), 0.0)
                    ) AS pending_balance,
                    (
                      COALESCE((SELECT SUM(balance) FROM monthly_tuition WHERE student_id = s.student_id AND (year < ?1 OR (year = ?1 AND month < ?2))), 0.0) +
                      COALESCE((SELECT SUM(balance) FROM monthly_bus_usage WHERE student_id = s.student_id AND (year < ?1 OR (year = ?1 AND month < ?2))), 0.0)
                    ) AS pending_past,
                    (
                      COALESCE((SELECT SUM(balance) FROM monthly_tuition WHERE student_id = s.student_id AND year = ?1 AND month = ?2), 0.0) +
                      COALESCE((SELECT SUM(balance) FROM monthly_bus_usage WHERE student_id = s.student_id AND year = ?1 AND month = ?2), 0.0)
                    ) AS pending_current
             FROM students s
             LEFT JOIN classes c ON s.class_id = c.id
             LEFT JOIN bus_stops b ON s.bus_stop_id = b.id
             WHERE s.student_id = ?4 OR s.student_name LIKE ?3 OR s.admission_number LIKE ?3 OR s.phone LIKE ?3 OR c.name LIKE ?3
             ORDER BY s.student_name ASC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![curr_year, curr_month, search_pattern, query], |r| {
            Ok(Student {
                id: r.get(0)?,
                student_id: r.get(1)?,
                admission_number: r.get(2)?,
                roll_number: r.get(3)?,
                student_name: r.get(4)?,
                parent_name: r.get(5)?,
                phone: r.get(6)?,
                class_id: r.get(7)?,
                class_name: r.get(8)?,
                bus_stop_id: r.get(9)?,
                bus_stop_name: r.get(10)?,
                status: r.get(11)?,
                academic_year_id: r.get(12)?,
                student_type: r.get(13)?,
                pending_balance: Some(r.get(14)?),
                pending_past: Some(r.get(15)?),
                pending_current: Some(r.get(16)?),
            })
        })
        .map_err(|e| e.to_string())?;

    let mut list = Vec::new();
    for r in rows {
        if let Ok(s) = r {
            list.push(s);
        }
    }
    Ok(list)
}

#[tauri::command]
pub fn promote_students(
    from_year_id: i64,
    to_year_id: i64,
    class_mapping: Vec<serde_json::Value>, // [{from_class_id: 1, to_class_id: 2}]
    carry_forward_balance: bool,
) -> Result<(), String> {
    let mut db = get_db().lock().unwrap();
    let tx = db.transaction().map_err(|e| e.to_string())?;
    let mut student_ids_to_sync = Vec::new();

    for mapping in class_mapping {
        let from_class_id = mapping["from_class_id"].as_i64().ok_or("Invalid from_class_id")?;
        let to_class_id = mapping["to_class_id"].as_i64().ok_or("Invalid to_class_id")?;

        let mut stmt = tx
            .prepare("SELECT student_id FROM students WHERE class_id = ? AND academic_year_id = ? AND status = 'active'")
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![from_class_id, from_year_id], |r| {
                r.get::<_, String>(0)
            })
            .map_err(|e| e.to_string())?;

        let mut students_to_promote = Vec::new();
        for r in rows {
            if let Ok(s_id) = r {
                students_to_promote.push(s_id);
            }
        }

        for student_id in students_to_promote {
            let prev_balance = if carry_forward_balance {
                tx.query_row(
                    "SELECT balance FROM bills WHERE student_id = ? AND academic_year_id = ?",
                    params![student_id, from_year_id],
                    |r| r.get::<_, f64>(0),
                )
                .unwrap_or(0.0)
            } else {
                0.0
            };

            // Update student's class and academic year to the promoted year
            tx.execute(
                "UPDATE students SET class_id = ?, academic_year_id = ?, updated_at = datetime('now') WHERE student_id = ?",
                params![to_class_id, to_year_id, student_id],
            )
            .map_err(|e| e.to_string())?;

            // Create default bill for the new academic year if carry-forward balance exists
            if prev_balance > 0.0 {
                tx.execute(
                    "INSERT OR REPLACE INTO bills (student_id, academic_year_id, previous_balance, total_fee, balance, payment_status, updated_at)
                     VALUES (?, ?, ?, ?, ?, 'pending', datetime('now'))",
                    params![student_id, to_year_id, prev_balance, prev_balance, prev_balance],
                )
                .map_err(|e| e.to_string())?;
            }

            student_ids_to_sync.push(student_id.clone());
        }
    }

    tx.commit().map_err(|e| e.to_string())?;
    drop(db);
    for student_id in student_ids_to_sync {
        let _ = enqueue_student_sync(&student_id);
    }
    Ok(())
}

#[tauri::command]
pub fn open_external_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    log::info!("Opening external URL: {}", url);
    app.opener().open_path(&url, None::<String>).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use std::fs;

    fn wipe_tables(conn: &rusqlite::Connection) {
        conn.execute_batch(
            "DELETE FROM receipts; DELETE FROM payments; DELETE FROM bill_items;
             DELETE FROM bills; DELETE FROM students; DELETE FROM classes;
             DELETE FROM bus_stops; DELETE FROM academic_years; DELETE FROM sync_queue;"
        ).unwrap();
    }

    #[test]
    fn test_all_fee_and_promotion_scenarios() {
        let test_dir = "target/test_shared_db";
        let _ = fs::remove_dir_all(test_dir);
        fs::create_dir_all(test_dir).unwrap();
        db::init_db(test_dir).unwrap();
        let conn_mutex = db::get_db();

        // ================================================================
        // SCENARIO 1: record_payment -> revert_payment
        // ================================================================
        {
            let conn = conn_mutex.lock().unwrap();
            wipe_tables(&conn);
            conn.execute(
                "INSERT INTO academic_years (name, start_date, end_date, is_active) VALUES ('2026-2027', '2026-04-01', '2027-03-31', 1)",
                params![],
            ).unwrap();
            let year_id: i64 = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO classes (name, section, tuition_fee, academic_year_id) VALUES ('Grade 1', 'A', 5000.0, ?)",
                params![year_id],
            ).unwrap();
            let class_id: i64 = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO students (student_id, admission_number, student_name, class_id, academic_year_id, status) VALUES ('STD001', 'ADM001', 'John Doe', ?, ?, 'active')",
                params![class_id, year_id],
            ).unwrap();
            conn.execute(
                "INSERT INTO bills (student_id, academic_year_id, total_fee, amount_paid, balance, payment_status) VALUES ('STD001', ?, 5000.0, 5000.0, 0.0, 'paid')",
                params![year_id],
            ).unwrap();
            let bill_id: i64 = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO payments (bill_id, student_id, amount, payment_mode, payment_date, receipt_number) VALUES (?, 'STD001', 5000.0, 'cash', '2026-07-07', 'REC-001')",
                params![bill_id],
            ).unwrap();
            let payment_id: i64 = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO receipts (receipt_number, payment_id) VALUES ('REC-001', ?)",
                params![payment_id],
            ).unwrap();
            drop(conn);

            revert_payment(payment_id).unwrap();

            let conn = conn_mutex.lock().unwrap();
            let payment_exists: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM payments WHERE id = ?)", params![payment_id], |r| r.get(0),
            ).unwrap();
            assert!(!payment_exists, "[S1] Payment should be deleted");
            let (amount_paid, balance, status): (f64, f64, String) = conn.query_row(
                "SELECT amount_paid, balance, payment_status FROM bills WHERE id = ?",
                params![bill_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            ).unwrap();
            assert_eq!(amount_paid, 0.0, "[S1] amount_paid should be 0");
            assert_eq!(balance, 5000.0, "[S1] balance restored");
            assert_eq!(status, "pending", "[S1] status pending");
        }

        // ================================================================
        // SCENARIO 2: generate_bill -> partial -> full payment
        // ================================================================
        {
            let conn = conn_mutex.lock().unwrap();
            wipe_tables(&conn);
            conn.execute(
                "INSERT INTO academic_years (name, start_date, end_date, is_active) VALUES ('2026-2027', '2026-04-01', '2027-03-31', 1)",
                params![],
            ).unwrap();
            let year_id: i64 = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO classes (name, section, tuition_fee, academic_year_id) VALUES ('Grade 2', 'B', 6000.0, ?)",
                params![year_id],
            ).unwrap();
            let class_id: i64 = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO students (student_id, admission_number, student_name, class_id, academic_year_id, status) VALUES ('STD_FEE', 'ADM_FEE', 'Jane Doe', ?, ?, 'active')",
                params![class_id, year_id],
            ).unwrap();
            drop(conn);

            generate_bill(GenerateBillRequest {
                student_id: "STD_FEE".to_string(),
                academic_year_id: year_id,
                tuition_fee: 6000.0, admission_fee: 500.0, exam_fee: 300.0,
                book_fee: 200.0, uniform_fee: 0.0, lab_fee: 0.0, computer_fee: 0.0,
                sports_fee: 0.0, activity_fee: 0.0, maintenance_fee: 0.0, bus_fee: 0.0,
                previous_balance: 0.0, extra_fees: 0.0, discount: 100.0, scholarship: 0.0,
                total_fee: 6900.0,
                extra_fee_items: vec![],
            }).unwrap();

            let conn = conn_mutex.lock().unwrap();
            let (total, balance, status): (f64, f64, String) = conn.query_row(
                "SELECT total_fee, balance, payment_status FROM bills WHERE student_id = 'STD_FEE'",
                params![], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            ).unwrap();
            assert_eq!(total, 6900.0, "[S2] Total fee 6900");
            assert_eq!(balance, 6900.0, "[S2] Balance equals total");
            assert_eq!(status, "pending", "[S2] Status pending");
            let bill_id: i64 = conn.query_row(
                "SELECT id FROM bills WHERE student_id = 'STD_FEE'", params![], |r| r.get(0),
            ).unwrap();
            drop(conn);

            let receipt = record_payment(RecordPaymentRequest {
                student_id: "STD_FEE".to_string(), bill_id, amount: 3000.0,
                payment_mode: "cash".to_string(), payment_date: "2026-07-07".to_string(), notes: None,
            }).unwrap();
            assert!(!receipt.is_empty(), "[S2] Receipt not empty");

            let conn = conn_mutex.lock().unwrap();
            let (paid, bal, status): (f64, f64, String) = conn.query_row(
                "SELECT amount_paid, balance, payment_status FROM bills WHERE id = ?",
                params![bill_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            ).unwrap();
            assert_eq!(paid, 3000.0, "[S2] Partial paid");
            assert_eq!(bal, 3900.0, "[S2] Partial balance");
            assert_eq!(status, "partial", "[S2] Status partial");
            drop(conn);

            record_payment(RecordPaymentRequest {
                student_id: "STD_FEE".to_string(), bill_id, amount: 3900.0,
                payment_mode: "online".to_string(), payment_date: "2026-07-08".to_string(), notes: None,
            }).unwrap();

            let conn = conn_mutex.lock().unwrap();
            let (paid, bal, status): (f64, f64, String) = conn.query_row(
                "SELECT amount_paid, balance, payment_status FROM bills WHERE id = ?",
                params![bill_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            ).unwrap();
            assert_eq!(paid, 6900.0, "[S2] Fully paid");
            assert_eq!(bal, 0.0, "[S2] Zero balance");
            assert_eq!(status, "paid", "[S2] Status paid");
        }

        // ================================================================
        // SCENARIO 3: promote_students with carry-forward balance
        // ================================================================
        {
            let conn = conn_mutex.lock().unwrap();
            wipe_tables(&conn);
            conn.execute(
                "INSERT INTO academic_years (name, start_date, end_date, is_active) VALUES ('2025-2026', '2025-04-01', '2026-03-31', 1)",
                params![],
            ).unwrap();
            let year1_id: i64 = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO academic_years (name, start_date, end_date, is_active) VALUES ('2026-2027', '2026-04-01', '2027-03-31', 0)",
                params![],
            ).unwrap();
            let year2_id: i64 = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO classes (name, section, tuition_fee, academic_year_id) VALUES ('Grade 1', 'A', 5000.0, ?)",
                params![year1_id],
            ).unwrap();
            let class1_id: i64 = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO classes (name, section, tuition_fee, academic_year_id) VALUES ('Grade 2', 'A', 6000.0, ?)",
                params![year2_id],
            ).unwrap();
            let class2_id: i64 = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO students (student_id, admission_number, student_name, class_id, academic_year_id, status) VALUES ('STD_PROM', 'ADM_PROM', 'Test Student', ?, ?, 'active')",
                params![class1_id, year1_id],
            ).unwrap();
            conn.execute(
                "INSERT INTO bills (student_id, academic_year_id, total_fee, amount_paid, balance, payment_status) VALUES ('STD_PROM', ?, 5000.0, 3000.0, 2000.0, 'partial')",
                params![year1_id],
            ).unwrap();
            drop(conn);

            let mapping = vec![serde_json::json!({"from_class_id": class1_id, "to_class_id": class2_id})];
            promote_students(year1_id, year2_id, mapping, true).unwrap();

            let conn = conn_mutex.lock().unwrap();
            let (new_class, new_year): (i64, i64) = conn.query_row(
                "SELECT class_id, academic_year_id FROM students WHERE student_id = 'STD_PROM'",
                params![], |r| Ok((r.get(0)?, r.get(1)?)),
            ).unwrap();
            assert_eq!(new_class, class2_id, "[S3] Promoted to Grade 2");
            assert_eq!(new_year, year2_id, "[S3] Year updated");

            let (prev_bal, total, bal, status): (f64, f64, f64, String) = conn.query_row(
                "SELECT previous_balance, total_fee, balance, payment_status FROM bills WHERE student_id = 'STD_PROM' AND academic_year_id = ?",
                params![year2_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            ).unwrap();
            assert_eq!(prev_bal, 2000.0, "[S3] Carry-forward correct");
            assert_eq!(total, 2000.0, "[S3] New bill total");
            assert_eq!(bal, 2000.0, "[S3] Balance");
            assert_eq!(status, "pending", "[S3] Status pending");
        }

        // ================================================================
        // SCENARIO 4: Monthly billing tuition/bus generation, payment, and reversion
        // ================================================================
        {
            let conn = conn_mutex.lock().unwrap();
            wipe_tables(&conn);
            conn.execute(
                "INSERT INTO academic_years (name, start_date, end_date, is_active) VALUES ('2026-2027', '2026-06-01', '2027-03-31', 1)",
                params![],
            ).unwrap();
            let ay_id: i64 = conn.last_insert_rowid();
            
            conn.execute(
                "INSERT INTO academic_year_months (academic_year_id, start_month, end_month, start_year) VALUES (?, 6, 3, 2026)",
                params![ay_id],
            ).unwrap();

            conn.execute(
                "INSERT INTO classes (name, section, tuition_fee, academic_year_id) VALUES ('Grade 3', 'A', 10000.0, ?)",
                params![ay_id],
            ).unwrap();
            let class_id: i64 = conn.last_insert_rowid();

            conn.execute(
                "INSERT INTO students (student_id, admission_number, student_name, class_id, academic_year_id, status) VALUES ('STD_MON', 'ADM_MON', 'Bob Smith', ?, ?, 'active')",
                params![class_id, ay_id],
            ).unwrap();
            
            drop(conn);
            
            generate_monthly_tuition_for_student("STD_MON".to_string(), ay_id).unwrap();
            
            let conn = conn_mutex.lock().unwrap();
            conn.execute(
                "INSERT INTO bus_stops (name, monthly_charge) VALUES ('Stop A', 500.0)",
                params![],
            ).unwrap();
            let stop_id: i64 = conn.last_insert_rowid();
            
            conn.execute(
                "UPDATE students SET bus_stop_id = ? WHERE student_id = 'STD_MON'",
                params![stop_id],
            ).unwrap();
            
            drop(conn);
            set_monthly_bus_usage("STD_MON".to_string(), ay_id, 6, 2026, true).unwrap();
            set_monthly_bus_usage("STD_MON".to_string(), ay_id, 7, 2026, true).unwrap();
            
            let receipt = record_monthly_payment(
                "STD_MON".to_string(),
                ay_id,
                2500.0,
                750.0,
                "cash".to_string(),
                "2026-07-08".to_string(),
                None,
            ).unwrap();
            
            let conn = conn_mutex.lock().unwrap();
            
            let (t_paid_6, t_bal_6, t_status_6): (f64, f64, String) = conn.query_row(
                "SELECT amount_paid, balance, status FROM monthly_tuition WHERE student_id = 'STD_MON' AND month = 6 AND year = 2026",
                params![], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            ).unwrap();
            assert_eq!(t_paid_6, 1000.0);
            assert_eq!(t_bal_6, 0.0);
            assert_eq!(t_status_6, "paid");

            let (t_paid_8, t_bal_8, t_status_8): (f64, f64, String) = conn.query_row(
                "SELECT amount_paid, balance, status FROM monthly_tuition WHERE student_id = 'STD_MON' AND month = 8 AND year = 2026",
                params![], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            ).unwrap();
            assert_eq!(t_paid_8, 500.0);
            assert_eq!(t_bal_8, 500.0);
            assert_eq!(t_status_8, "partial");

            let (b_paid_6, b_bal_6, b_status_6): (f64, f64, String) = conn.query_row(
                "SELECT amount_paid, balance, status FROM monthly_bus_usage WHERE student_id = 'STD_MON' AND month = 6 AND year = 2026",
                params![], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            ).unwrap();
            assert_eq!(b_paid_6, 500.0);
            assert_eq!(b_bal_6, 0.0);
            assert_eq!(b_status_6, "paid");

            let (b_paid_7, b_bal_7, b_status_7): (f64, f64, String) = conn.query_row(
                "SELECT amount_paid, balance, status FROM monthly_bus_usage WHERE student_id = 'STD_MON' AND month = 7 AND year = 2026",
                params![], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            ).unwrap();
            assert_eq!(b_paid_7, 250.0);
            assert_eq!(b_bal_7, 250.0);
            assert_eq!(b_status_7, "partial");

            let payment_id: i64 = conn.query_row(
                "SELECT id FROM payments WHERE receipt_number = ?",
                params![receipt], |r| r.get(0),
            ).unwrap();
            
            drop(conn);

            revert_payment(payment_id).unwrap();

            let conn = conn_mutex.lock().unwrap();
            let (t_paid_6_rev, t_bal_6_rev, t_status_6_rev): (f64, f64, String) = conn.query_row(
                "SELECT amount_paid, balance, status FROM monthly_tuition WHERE student_id = 'STD_MON' AND month = 6 AND year = 2026",
                params![], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            ).unwrap();
            assert_eq!(t_paid_6_rev, 0.0);
            assert_eq!(t_bal_6_rev, 1000.0);
            assert_eq!(t_status_6_rev, "unpaid");

            let (b_paid_6_rev, b_bal_6_rev, b_status_6_rev): (f64, f64, String) = conn.query_row(
                "SELECT amount_paid, balance, status FROM monthly_bus_usage WHERE student_id = 'STD_MON' AND month = 6 AND year = 2026",
                params![], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            ).unwrap();
            assert_eq!(b_paid_6_rev, 0.0);
            assert_eq!(b_bal_6_rev, 500.0);
            assert_eq!(b_status_6_rev, "unpaid");
        }

        let _ = fs::remove_dir_all(test_dir);
    }
}
