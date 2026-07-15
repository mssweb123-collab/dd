use anyhow::Result;
use once_cell::sync::OnceCell;
use rusqlite::{Connection, params};
use std::sync::Mutex;
use log::info;

static DB: OnceCell<Mutex<Connection>> = OnceCell::new();

pub fn get_db() -> &'static Mutex<Connection> {
    DB.get().expect("Database not initialized")
}

pub fn init_db(app_data_dir: &str) -> Result<()> {
    let db_path = format!("{}/mss_billing.db", app_data_dir);
    info!("Opening SQLite database at: {}", db_path);
    let conn = Connection::open(&db_path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;")?;
    run_migrations(&conn)?;
    DB.set(Mutex::new(conn)).map_err(|_| anyhow::anyhow!("DB already initialized"))?;
    info!("Database initialized successfully");
    Ok(())
}

fn run_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT UNIQUE NOT NULL,
            password_hash TEXT NOT NULL,
            role TEXT NOT NULL CHECK(role IN ('admin', 'accountant')),
            created_at TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS school_settings (
            id INTEGER PRIMARY KEY CHECK(id = 1),
            school_name TEXT NOT NULL DEFAULT 'My School',
            logo_path TEXT,
            address TEXT,
            phone TEXT,
            receipt_footer TEXT,
            printer_name TEXT,
            updated_at TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS academic_years (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT UNIQUE NOT NULL,
            start_date TEXT NOT NULL,
            end_date TEXT,
            is_active INTEGER DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS classes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            section TEXT,
            tuition_fee REAL DEFAULT 0,
            admission_fee REAL DEFAULT 0,
            exam_fee REAL DEFAULT 0,
            book_fee REAL DEFAULT 0,
            uniform_fee REAL DEFAULT 0,
            lab_fee REAL DEFAULT 0,
            computer_fee REAL DEFAULT 0,
            sports_fee REAL DEFAULT 0,
            activity_fee REAL DEFAULT 0,
            maintenance_fee REAL DEFAULT 0,
            academic_year_id INTEGER REFERENCES academic_years(id),
            created_at TEXT DEFAULT (datetime('now')),
            UNIQUE(name, section, academic_year_id)
        );

        CREATE TABLE IF NOT EXISTS fee_components (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            class_id INTEGER NOT NULL REFERENCES classes(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            amount REAL NOT NULL DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS bus_stops (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT UNIQUE NOT NULL,
            monthly_charge REAL NOT NULL DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS students (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            student_id TEXT UNIQUE NOT NULL,
            admission_number TEXT UNIQUE NOT NULL,
            roll_number TEXT,
            student_name TEXT NOT NULL,
            parent_name TEXT,
            phone TEXT,
            class_id INTEGER REFERENCES classes(id),
            bus_stop_id INTEGER REFERENCES bus_stops(id),
            status TEXT DEFAULT 'active' CHECK(status IN ('active', 'inactive', 'transferred')),
            academic_year_id INTEGER REFERENCES academic_years(id),
            student_type TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS bills (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            student_id TEXT NOT NULL,
            academic_year_id INTEGER NOT NULL REFERENCES academic_years(id),
            tuition_fee REAL DEFAULT 0,
            admission_fee REAL DEFAULT 0,
            exam_fee REAL DEFAULT 0,
            book_fee REAL DEFAULT 0,
            uniform_fee REAL DEFAULT 0,
            lab_fee REAL DEFAULT 0,
            computer_fee REAL DEFAULT 0,
            sports_fee REAL DEFAULT 0,
            activity_fee REAL DEFAULT 0,
            maintenance_fee REAL DEFAULT 0,
            bus_fee REAL DEFAULT 0,
            previous_balance REAL DEFAULT 0,
            extra_fees REAL DEFAULT 0,
            discount REAL DEFAULT 0,
            scholarship REAL DEFAULT 0,
            total_fee REAL NOT NULL DEFAULT 0,
            amount_paid REAL DEFAULT 0,
            balance REAL DEFAULT 0,
            payment_status TEXT DEFAULT 'pending' CHECK(payment_status IN ('pending', 'partial', 'paid')),
            last_payment_date TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now')),
            UNIQUE(student_id, academic_year_id)
        );

        CREATE TABLE IF NOT EXISTS bill_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            bill_id INTEGER NOT NULL REFERENCES bills(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            amount REAL NOT NULL
        );

        CREATE TABLE IF NOT EXISTS payments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            student_id TEXT NOT NULL,
            bill_id INTEGER REFERENCES bills(id),
            payment_date TEXT NOT NULL DEFAULT (date('now')),
            amount REAL NOT NULL,
            payment_mode TEXT DEFAULT 'cash' CHECK(payment_mode IN ('cash', 'online', 'cheque', 'dd')),
            receipt_number TEXT UNIQUE NOT NULL,
            academic_year_id INTEGER REFERENCES academic_years(id),
            notes TEXT,
            allocated_admission REAL DEFAULT 0,
            allocated_other REAL DEFAULT 0,
            allocated_tuition REAL DEFAULT 0,
            allocated_bus REAL DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS receipts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            payment_id INTEGER NOT NULL REFERENCES payments(id),
            receipt_number TEXT NOT NULL,
            generated_at TEXT DEFAULT (datetime('now')),
            printed_at TEXT
        );

        CREATE TABLE IF NOT EXISTS sync_queue (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            student_id TEXT NOT NULL,
            payload TEXT NOT NULL,
            status TEXT DEFAULT 'pending' CHECK(status IN ('pending', 'syncing', 'success', 'failed')),
            retry_count INTEGER DEFAULT 0,
            last_attempt_at TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            error_message TEXT
        );

        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT DEFAULT (datetime('now'))
        );

        INSERT OR IGNORE INTO school_settings (id, school_name) VALUES (1, 'MSS School');
        INSERT OR IGNORE INTO users (username, password_hash, role) VALUES ('admin', 'admin123', 'admin');
        INSERT OR IGNORE INTO users (username, password_hash, role) VALUES ('accountant', 'acc123', 'accountant');
        INSERT OR IGNORE INTO settings (key, value) VALUES ('theme', 'dark');
        INSERT OR IGNORE INTO settings (key, value) VALUES ('receipt_type', 'a4');
        INSERT OR IGNORE INTO settings (key, value) VALUES ('last_sync', '');
        INSERT OR IGNORE INTO settings (key, value) VALUES ('receipt_counter', '1');
    ")?;

    // Add supabase_id to classes table if it doesn't exist
    let _ = conn.execute("ALTER TABLE classes ADD COLUMN supabase_id TEXT;", []);
    
    // Add student_type to students table if it doesn't exist
    let _ = conn.execute("ALTER TABLE students ADD COLUMN student_type TEXT;", []);

    // Add allocation fields to payments table if they don't exist
    let _ = conn.execute("ALTER TABLE payments ADD COLUMN allocated_admission REAL DEFAULT 0;", []);
    let _ = conn.execute("ALTER TABLE payments ADD COLUMN allocated_other REAL DEFAULT 0;", []);
    let _ = conn.execute("ALTER TABLE payments ADD COLUMN allocated_tuition REAL DEFAULT 0;", []);
    let _ = conn.execute("ALTER TABLE payments ADD COLUMN allocated_bus REAL DEFAULT 0;", []);

    // Create a unique index on supabase_id to ensure fast lookups and prevent duplicates
    let _ = conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_classes_supabase_id ON classes(supabase_id);",
        [],
    );

    // Migration: make end_date nullable (safe on existing DBs)
    let _ = conn.execute_batch("
        CREATE TABLE IF NOT EXISTS academic_years_new (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT UNIQUE NOT NULL,
            start_date TEXT NOT NULL,
            end_date TEXT,
            is_active INTEGER DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now'))
        );
        INSERT OR IGNORE INTO academic_years_new (id, name, start_date, end_date, is_active, created_at)
            SELECT id, name, start_date, end_date, is_active, created_at FROM academic_years;
        DROP TABLE IF EXISTS academic_years;
        ALTER TABLE academic_years_new RENAME TO academic_years;
    ");

    // ── MONTHLY TUITION & BUS SYSTEM MIGRATIONS ─────────────────────────────

    // Academic year month range config (e.g. June → March = 10 months)
    let _ = conn.execute_batch("
        CREATE TABLE IF NOT EXISTS academic_year_months (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            academic_year_id INTEGER NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
            start_month     INTEGER NOT NULL DEFAULT 6,
            end_month       INTEGER NOT NULL DEFAULT 3,
            start_year      INTEGER NOT NULL DEFAULT 2026,
            UNIQUE(academic_year_id)
        );
    ");

    // Monthly tuition installments per student per month
    let _ = conn.execute_batch("
        CREATE TABLE IF NOT EXISTS monthly_tuition (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            student_id       TEXT NOT NULL,
            academic_year_id INTEGER NOT NULL REFERENCES academic_years(id),
            month            INTEGER NOT NULL CHECK(month BETWEEN 1 AND 12),
            year             INTEGER NOT NULL,
            monthly_amount   REAL NOT NULL DEFAULT 0,
            amount_paid      REAL NOT NULL DEFAULT 0,
            balance          REAL NOT NULL DEFAULT 0,
            status           TEXT NOT NULL DEFAULT 'unpaid'
                                 CHECK(status IN ('paid','partial','unpaid')),
            created_at       TEXT DEFAULT (datetime('now')),
            updated_at       TEXT DEFAULT (datetime('now')),
            UNIQUE(student_id, academic_year_id, month, year)
        );
        CREATE INDEX IF NOT EXISTS idx_mt_student_ay
            ON monthly_tuition(student_id, academic_year_id);
        CREATE INDEX IF NOT EXISTS idx_mt_status
            ON monthly_tuition(status);
    ");

    // Monthly bus usage per student per month
    let _ = conn.execute_batch("
        CREATE TABLE IF NOT EXISTS monthly_bus_usage (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            student_id       TEXT NOT NULL,
            academic_year_id INTEGER NOT NULL REFERENCES academic_years(id),
            month            INTEGER NOT NULL CHECK(month BETWEEN 1 AND 12),
            year             INTEGER NOT NULL,
            bus_used         INTEGER NOT NULL DEFAULT 0,
            bus_fee          REAL NOT NULL DEFAULT 0,
            amount_paid      REAL NOT NULL DEFAULT 0,
            balance          REAL NOT NULL DEFAULT 0,
            status           TEXT NOT NULL DEFAULT 'unpaid'
                                 CHECK(status IN ('paid','partial','unpaid')),
            created_at       TEXT DEFAULT (datetime('now')),
            UNIQUE(student_id, academic_year_id, month, year)
        );
        CREATE INDEX IF NOT EXISTS idx_mbu_student_ay
            ON monthly_bus_usage(student_id, academic_year_id);
    ");

    // Auto-add bus_fee_mode setting
    let _ = conn.execute(
        "INSERT OR IGNORE INTO settings (key, value) VALUES ('bus_fee_mode', 'ask')",
        [],
    );

    info!("Database migrations completed");
    Ok(())
}

pub fn next_receipt_number(conn: &Connection) -> Result<String> {
    let counter: i64 = conn.query_row(
        "SELECT CAST(value AS INTEGER) FROM settings WHERE key = 'receipt_counter'",
        [],
        |row| row.get(0),
    ).unwrap_or(1);
    let receipt_no = format!("RCP{:06}", counter);
    conn.execute(
        "UPDATE settings SET value = ?, updated_at = datetime('now') WHERE key = 'receipt_counter'",
        params![counter + 1],
    )?;
    Ok(receipt_no)
}
