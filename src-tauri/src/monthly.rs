use anyhow::Result;
use rusqlite::params;
use log::info;

/// Number of months from start_month to end_month inclusive, wrapping across year boundary.
/// e.g. June(6) → March(3) = 10 months; Jan(1) → Dec(12) = 12 months
pub fn academic_month_count(start_month: u32, end_month: u32) -> u32 {
    if end_month >= start_month {
        end_month - start_month + 1
    } else {
        12 - start_month + end_month + 1
    }
}

/// Given an academic year's start_month/start_year, return an ordered list of (month, calendar_year).
pub fn academic_month_list(start_month: u32, start_year: i32, num_months: u32) -> Vec<(u32, i32)> {
    let mut list = Vec::with_capacity(num_months as usize);
    let mut m = start_month;
    let mut y = start_year;
    for _ in 0..num_months {
        list.push((m, y));
        m += 1;
        if m > 12 {
            m = 1;
            y += 1;
        }
    }
    list
}

// ─────────────────────────────────────────────────────────────────────────────
// GENERATE monthly_tuition rows for ONE student (idempotent INSERT OR IGNORE)
// ─────────────────────────────────────────────────────────────────────────────
pub fn generate_monthly_tuition_for_student(
    db: &rusqlite::Connection,
    student_id: &str,
    academic_year_id: i64,
    yearly_tuition: f64,
) -> Result<()> {
    // Get year config
    let (start_month, end_month, ay_start_date): (u32, u32, String) = db
        .query_row(
            "SELECT aym.start_month, aym.end_month, ay.start_date
             FROM academic_year_months aym
             JOIN academic_years ay ON ay.id = aym.academic_year_id
             WHERE aym.academic_year_id = ?",
            params![academic_year_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap_or((6, 3, "2026-06-01".to_string())); // default June→March

    // Parse start year from start_date
    let start_year: i32 = ay_start_date
        .split('-')
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2026);

    let num_months = academic_month_count(start_month, end_month).max(1);
    let monthly_amount = yearly_tuition / num_months as f64;

    let months = academic_month_list(start_month, start_year, num_months);

    for (month, year) in months {
        db.execute(
            "INSERT INTO monthly_tuition
             (student_id, academic_year_id, month, year, monthly_amount, amount_paid, balance, status)
             VALUES (?, ?, ?, ?, ?, 0, ?, 'unpaid')
             ON CONFLICT(student_id, academic_year_id, month, year)
             DO UPDATE SET
               monthly_amount = excluded.monthly_amount,
               amount_paid = MIN(amount_paid, excluded.monthly_amount),
               balance = MAX(0.0, excluded.monthly_amount - MIN(amount_paid, excluded.monthly_amount)),
               status = CASE
                 WHEN MAX(0.0, excluded.monthly_amount - MIN(amount_paid, excluded.monthly_amount)) <= 0.001 THEN 'paid'
                 WHEN MIN(amount_paid, excluded.monthly_amount) > 0 THEN 'partial'
                 ELSE 'unpaid'
               END",
            params![student_id, academic_year_id, month, year, monthly_amount, monthly_amount],
        )?;
    }

    info!(
        "Generated {} monthly tuition records for student {}",
        num_months, student_id
    );
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// APPLY monthly payment: distribute amount across unpaid months oldest-first
// ─────────────────────────────────────────────────────────────────────────────
pub fn apply_monthly_tuition_payment(
    db: &rusqlite::Connection,
    student_id: &str,
    academic_year_id: i64,
    mut amount: f64,
) -> Result<()> {
    // Get unpaid/partial months oldest first
    let mut stmt = db.prepare(
        "SELECT id, balance FROM monthly_tuition
         WHERE student_id = ? AND academic_year_id = ?
           AND status IN ('unpaid', 'partial')
         ORDER BY year ASC, month ASC",
    )?;

    let rows: Vec<(i64, f64)> = stmt
        .query_map(params![student_id, academic_year_id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

    for (id, balance) in rows {
        if amount <= 0.0 {
            break;
        }
        let pay = amount.min(balance);
        let new_balance = (balance - pay).max(0.0);
        let new_status = if new_balance <= 0.001 { "paid" } else { "partial" };

        db.execute(
            "UPDATE monthly_tuition
             SET amount_paid = amount_paid + ?,
                 balance = ?,
                 status = ?,
                 updated_at = datetime('now')
             WHERE id = ?",
            params![pay, new_balance, new_status, id],
        )?;
        amount -= pay;
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// APPLY monthly bus payment
// ─────────────────────────────────────────────────────────────────────────────
pub fn apply_monthly_bus_payment(
    db: &rusqlite::Connection,
    student_id: &str,
    academic_year_id: i64,
    mut amount: f64,
) -> Result<()> {
    let mut stmt = db.prepare(
        "SELECT id, balance FROM monthly_bus_usage
         WHERE student_id = ? AND academic_year_id = ?
           AND bus_used = 1 AND status IN ('unpaid', 'partial')
         ORDER BY year ASC, month ASC",
    )?;

    let rows: Vec<(i64, f64)> = stmt
        .query_map(params![student_id, academic_year_id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

    for (id, balance) in rows {
        if amount <= 0.0 {
            break;
        }
        let pay = amount.min(balance);
        let new_balance = (balance - pay).max(0.0);
        let new_status = if new_balance <= 0.001 { "paid" } else { "partial" };

        db.execute(
            "UPDATE monthly_bus_usage
             SET amount_paid = amount_paid + ?,
                 balance = ?,
                 status = ?
             WHERE id = ?",
            params![pay, new_balance, new_status, id],
        )?;
        amount -= pay;
    }
    Ok(())
}
