use crate::db::get_db;
use anyhow::Result;
use log::{error, info};
use reqwest::Client;
use rusqlite::params;
use serde_json::json;

pub async fn sync_to_supabase(supabase_url: &str, service_key: &str) -> Result<()> {
    // Reset failed items so they are retried, clearing out retry count
    {
        if let Ok(db) = get_db().lock() {
            // Dedup: for each student_id that has multiple pending rows, keep only the latest one.
            // This cleans up any stale duplicates left over from before the enqueue fix.
            let _ = db.execute(
                "DELETE FROM sync_queue
                 WHERE status IN ('pending', 'failed')
                   AND id NOT IN (
                       SELECT MAX(id) FROM sync_queue
                       WHERE status IN ('pending', 'failed')
                       GROUP BY student_id
                   )",
                [],
            );
            let _ = db.execute(
                "UPDATE sync_queue SET status='pending', retry_count=0, error_message=NULL WHERE status='failed'",
                [],
            );
        }
    }

    let client = Client::new();
    let pending = {
        let db = get_db().lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT id, student_id, payload FROM sync_queue WHERE status='pending' OR (status='failed' AND retry_count < 5) LIMIT 20"
        )?;
        let rows: Vec<(i64, String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .filter_map(|r| r.ok())
            .collect();
        rows
    };

    for (id, _student_id, payload) in pending {
        {
            let db = get_db().lock().unwrap();
            db.execute(
                "UPDATE sync_queue SET status='syncing', last_attempt_at=datetime('now') WHERE id=?",
                params![id],
            )?;
        }

        // Parse the payload object, then wrap it in a JSON array.
        // Supabase PostgREST requires an array for upsert (POST with Prefer: resolution=merge-duplicates).
        let mut obj: serde_json::Value = match serde_json::from_str(&payload) {
            Ok(v) => v,
            Err(e) => {
                let db = get_db().lock().unwrap();
                db.execute(
                    "UPDATE sync_queue SET status='failed', retry_count=retry_count+1, error_message=? WHERE id=?",
                    params![format!("Bad payload JSON: {}", e), id],
                )?;
                continue;
            }
        };

        // Remove balance_fee and map fields to Supabase schema
        if let Some(map) = obj.as_object_mut() {
            map.remove("balance_fee");
        }

        // Wrap single object in array as PostgREST requires
        let body = serde_json::Value::Array(vec![obj]);

        let url = format!("{}/rest/v1/student_billing?on_conflict=student_id", supabase_url);
        let res = client
            .post(&url)
            .header("apikey", service_key)
            .header("Authorization", format!("Bearer {}", service_key))
            .header("Content-Type", "application/json")
            // Tell PostgREST to upsert on the student_id column
            .header("Prefer", "resolution=merge-duplicates,return=minimal")
            .json(&body)
            .send()
            .await;

        // Resolve the response fully (including error body) BEFORE acquiring the DB lock,
        // because MutexGuard is !Send and cannot be held across an .await point.
        enum SyncResult {
            Success,
            Failed(String),
        }

        let sync_result = match res {
            Ok(r) if r.status().is_success() => SyncResult::Success,
            Ok(r) => {
                let status_code = r.status();
                let err_body = r.text().await.unwrap_or_default();
                SyncResult::Failed(format!("HTTP {} — {}", status_code, err_body))
            }
            Err(e) => SyncResult::Failed(e.to_string()),
        };

        let db = get_db().lock().unwrap();
        match sync_result {
            SyncResult::Success => {
                db.execute(
                    "UPDATE sync_queue SET status='success' WHERE id=?",
                    params![id],
                )?;
                db.execute(
                    "UPDATE settings SET value=datetime('now') WHERE key='last_sync'",
                    [],
                )?;
                info!("Synced queue item {}", id);
            }
            SyncResult::Failed(msg) => {
                db.execute(
                    "UPDATE sync_queue SET status='failed', retry_count=retry_count+1, error_message=? WHERE id=?",
                    params![msg, id],
                )?;
                error!("Sync failed for item {}: {}", id, msg);
            }
        }

    }
    Ok(())
}

pub fn enqueue_student_sync(student_id: &str) -> Result<()> {
    let db = get_db().lock().unwrap();

    // Query student name
    let student_name: String = db
        .query_row(
            "SELECT student_name FROM students WHERE student_id = ?",
            params![student_id],
            |r| r.get(0)
        )
        .unwrap_or_else(|_| "Unknown".to_string());

    // 1. Try querying from yearly bills first
    // 2. Compute grand totals for ALL fees across all years
    let grand_totals: (f64, f64, f64) = db
        .query_row(
            "SELECT 
                COALESCE((SELECT SUM(total_fee) FROM bills WHERE student_id = ?), 0.0) +
                COALESCE((SELECT SUM(monthly_amount) FROM monthly_tuition WHERE student_id = ?), 0.0) +
                COALESCE((SELECT SUM(bus_fee) FROM monthly_bus_usage WHERE student_id = ?), 0.0),
                
                COALESCE((SELECT SUM(amount_paid) FROM bills WHERE student_id = ?), 0.0) +
                COALESCE((SELECT SUM(amount_paid) FROM monthly_tuition WHERE student_id = ?), 0.0) +
                COALESCE((SELECT SUM(amount_paid) FROM monthly_bus_usage WHERE student_id = ?), 0.0),
                
                COALESCE((SELECT SUM(balance) FROM bills WHERE student_id = ?), 0.0) +
                COALESCE((SELECT SUM(balance) FROM monthly_tuition WHERE student_id = ?), 0.0) +
                COALESCE((SELECT SUM(balance) FROM monthly_bus_usage WHERE student_id = ?), 0.0)",
            params![student_id, student_id, student_id, student_id, student_id, student_id, student_id, student_id, student_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        )
        .unwrap_or((0.0, 0.0, 0.0));

    let (total, paid, balance) = grand_totals;

    // Get last payment date
    let last_dt: Option<String> = db
        .query_row(
            "SELECT payment_date FROM payments WHERE student_id = ? ORDER BY id DESC LIMIT 1",
            params![student_id],
            |r| r.get(0)
        )
        .ok();

    // Get the name of the last COMPLETELY PAID month (Tuition Fee ONLY)
    let last_paid_month: Option<i32> = db
        .query_row(
            "SELECT month FROM monthly_tuition 
             WHERE student_id = ? AND status = 'paid' 
             ORDER BY year DESC, month DESC LIMIT 1",
            params![student_id],
            |r| r.get(0)
        )
        .ok();

    let month_names = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    let last_paid_month_name = last_paid_month
        .and_then(|m| month_names.get((m-1) as usize))
        .map(|&s| s.to_string())
        .unwrap_or("Nil".to_string());

    // Build the payload mapping columns exactly to Supabase student_billing schema
    let last_dt_val = last_dt
        .filter(|s| !s.is_empty())
        .map(|s| serde_json::Value::String(s))
        .unwrap_or(serde_json::Value::Null);

    let status = if balance <= 0.01 && total > 0.0 {
        "paid"
    } else if paid > 0.01 {
        "partial"
    } else {
        "unpaid"
    };

    let payload = json!({
        "student_id": student_id,
        "student_name": student_name,
        "total_fee": total,
        "total_fee_paid": paid,
        "current_month_status": status,
        "last_paid_date": last_dt_val,
        "academic_year": last_paid_month_name,
        "updated_at": chrono::Utc::now().to_rfc3339()
    });

    // Remove any existing pending/failed entry for this student so we never
    // send duplicate payloads for the same student_id (would cause 409 on Supabase).
    db.execute(
        "DELETE FROM sync_queue WHERE student_id = ? AND status IN ('pending', 'failed')",
        params![student_id],
    )?;

    db.execute(
        "INSERT INTO sync_queue (student_id, payload, status) VALUES (?,?,'pending')",
        params![student_id, payload.to_string()],
    )?;
    Ok(())
}

use crate::models::{SupabaseClass, SupabaseStudent};
use rusqlite::OptionalExtension;

pub async fn sync_from_supabase(supabase_url: &str, service_key: &str) -> Result<()> {
    let client = Client::new();

    // 1. Fetch Class data from Supabase
    info!("Sync: Fetching classes from Supabase...");
    let classes_url = format!("{}/rest/v1/classes?select=*", supabase_url);
    let classes_res = client
        .get(&classes_url)
        .header("apikey", service_key)
        .header("Authorization", format!("Bearer {}", service_key))
        .send()
        .await?;

    if !classes_res.status().is_success() {
        let err_text = classes_res.text().await?;
        error!("Failed to fetch classes: {}", err_text);
        return Err(anyhow::anyhow!("Supabase class fetch failed: {}", err_text));
    }
    let supabase_classes: Vec<SupabaseClass> = classes_res.json().await?;
    info!("Sync: Found {} classes in Supabase", supabase_classes.len());

    // 2. Fetch Student data from Supabase
    info!("Sync: Fetching students from Supabase...");
    let students_url = format!("{}/rest/v1/students?select=*", supabase_url);
    let students_res = client
        .get(&students_url)
        .header("apikey", service_key)
        .header("Authorization", format!("Bearer {}", service_key))
        .send()
        .await?;

    if !students_res.status().is_success() {
        let err_text = students_res.text().await?;
        error!("Failed to fetch students: {}", err_text);
        return Err(anyhow::anyhow!("Supabase student fetch failed: {}", err_text));
    }
    let supabase_students: Vec<SupabaseStudent> = students_res.json().await?;
    info!("Sync: Found {} students in Supabase", supabase_students.len());

    // 3. Upsert into SQLite in a Transaction
    let mut db = get_db().lock().unwrap();
    let tx = db.transaction()?;

    // Get local active academic year (fallback if student lacks academic year)
    let active_ay_id: Option<i64> = tx
        .query_row(
            "SELECT id FROM academic_years WHERE is_active = 1",
            [],
            |r| r.get(0),
        )
        .optional()?;

    // Upsert Classes
    for class in supabase_classes {
        // A. Check if class exists by supabase_id
        let exists: Option<i64> = tx
            .query_row(
                "SELECT id FROM classes WHERE supabase_id = ?",
                params![class.id],
                |r| r.get(0),
            )
            .optional()?;

        if let Some(local_id) = exists {
            tx.execute(
                "UPDATE classes SET name = ?, section = ? WHERE id = ?",
                params![class.name, class.section, local_id],
            )?;
        } else {
            // B. Resolve conflict: if a class with the same name and section exists locally, link it
            // NULL-safe section comparison: `section = ?` is false when both sides are NULL in SQL
            let local_match: Option<i64> = tx
                .query_row(
                    "SELECT id FROM classes WHERE name = ? \
AND (section = ? OR (section IS NULL AND ? IS NULL)) \
AND (academic_year_id = ? OR academic_year_id IS NULL) LIMIT 1",
                    params![class.name, class.section, class.section, active_ay_id],
                    |r| r.get(0),
                )
                .optional()?;

            if let Some(lid) = local_match {
                tx.execute(
                    "UPDATE classes SET supabase_id = ? WHERE id = ?",
                    params![class.id, lid],
                )?;
            } else {
                // C. Otherwise, create a new local class
                tx.execute(
                    "INSERT INTO classes (name, section, academic_year_id, supabase_id) VALUES (?, ?, ?, ?)",
                    params![class.name, class.section, active_ay_id, class.id],
                )?;
            }
        }
    }

    // Upsert Students
    for student in supabase_students {
        // A. Map Supabase Class ID to SQLite Class ID (Integer)
        let local_class_id: Option<i64> = if let Some(ref sb_class_id) = student.class_id {
            tx.query_row(
                "SELECT id FROM classes WHERE supabase_id = ?",
                params![sb_class_id],
                |r| r.get(0),
            )
            .optional()?
        } else {
            None
        };

        // B. Map Student Academic Year string to SQLite Academic Year ID (Integer)
        let local_ay_id: Option<i64> = if let Some(ref ay_name) = student.academic_year {
            let ay_id: Option<i64> = tx
                .query_row(
                    "SELECT id FROM academic_years WHERE name = ?",
                    params![ay_name],
                    |r| r.get(0),
                )
                .optional()?;

            if ay_id.is_none() {
                // Automatically register the academic year if missing locally
                tx.execute(
                    "INSERT INTO academic_years (name, start_date, is_active) VALUES (?, '2026-06-01', 0)",
                    params![ay_name],
                )?;
                Some(tx.last_insert_rowid())
            } else {
                ay_id
            }
        } else {
            active_ay_id
        };

        // roll_no is nullable in Supabase; treat blank strings the same as null
        let roll: Option<String> = student.roll_no
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string());

        // local SQLite students table requires UNIQUE NOT NULL admission_number.
        // We use roll_no if present, otherwise fallback to student_id (Supabase UUID).
        let admission_number = roll.clone().unwrap_or_else(|| student.id.clone());

        // C. Check if student already exists by student_id
        // Also fetch existing bus_stop_id so accountant assignment is preserved
        let exists: Option<(i64, Option<i64>)> = tx
            .query_row(
                "SELECT id, bus_stop_id FROM students WHERE student_id = ?",
                params![student.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;

        // Whether Supabase marks this student as a bus student
        let is_bus_student = student.r#type.as_deref().map(|t| t == "bus").unwrap_or(false);

        if let Some((local_student_db_id, existing_bus_stop_id)) = exists {
            // Preserve bus_stop_id the accountant set; clear only if no longer a bus student
            let new_bus_stop_id: Option<i64> = if is_bus_student {
                existing_bus_stop_id // keep accountant's assignment
            } else {
                None
            };

            tx.execute(
                "UPDATE students SET
                    admission_number = ?,
                    roll_number = ?,
                    student_name = ?,
                    parent_name = ?,
                    phone = ?,
                    class_id = ?,
                    bus_stop_id = ?,
                    academic_year_id = ?,
                    student_type = ?,
                    updated_at = datetime('now')
                 WHERE id = ?",
                params![
                    admission_number,
                    roll,
                    student.name,
                    student.parent_name,
                    student.phone,
                    local_class_id,
                    new_bus_stop_id,
                    local_ay_id,
                    student.r#type,
                    local_student_db_id
                ],
            )?;
        } else {
            // New student: bus_stop_id starts null; accountant assigns in fee billing
            tx.execute(
                "INSERT INTO students (student_id, admission_number, roll_number, student_name, parent_name, phone, class_id, bus_stop_id, status, academic_year_id, student_type)
                 VALUES (?, ?, ?, ?, ?, ?, ?, NULL, 'active', ?, ?)",
                params![
                    student.id,
                    admission_number,
                    roll,
                    student.name,
                    student.parent_name,
                    student.phone,
                    local_class_id,
                    local_ay_id,
                    student.r#type
                ],
            )?;
        }
    }

    tx.commit()?;
    info!("Sync: Data synced from Supabase successfully");
    Ok(())
}

