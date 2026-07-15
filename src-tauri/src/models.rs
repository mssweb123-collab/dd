use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub role: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SchoolSettings {
    pub id: i64,
    pub school_name: String,
    pub logo_path: Option<String>,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub receipt_footer: Option<String>,
    pub printer_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AcademicYear {
    pub id: i64,
    pub name: String,
    pub start_date: String,
    pub end_date: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Class {
    pub id: i64,
    pub name: String,
    pub section: Option<String>,
    pub tuition_fee: f64,
    pub admission_fee: f64,
    pub exam_fee: f64,
    pub book_fee: f64,
    pub uniform_fee: f64,
    pub lab_fee: f64,
    pub computer_fee: f64,
    pub sports_fee: f64,
    pub activity_fee: f64,
    pub maintenance_fee: f64,
    pub academic_year_id: Option<i64>,
    pub custom_fees: Vec<FeeComponent>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeeComponent {
    pub id: i64,
    pub class_id: i64,
    pub name: String,
    pub amount: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BusStop {
    pub id: i64,
    pub name: String,
    pub monthly_charge: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Student {
    pub id: i64,
    pub student_id: String,
    pub admission_number: String,
    pub roll_number: Option<String>,
    pub student_name: String,
    pub parent_name: Option<String>,
    pub phone: Option<String>,
    pub class_id: Option<i64>,
    pub class_name: Option<String>,
    pub bus_stop_id: Option<i64>,
    pub bus_stop_name: Option<String>,
    pub status: String,
    pub academic_year_id: Option<i64>,
    pub pending_balance: Option<f64>,
    pub pending_past: Option<f64>,
    pub pending_current: Option<f64>,
    pub student_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Bill {
    pub id: i64,
    pub student_id: String,
    pub student_name: Option<String>,
    pub academic_year_id: i64,
    pub tuition_fee: f64,
    pub admission_fee: f64,
    pub exam_fee: f64,
    pub book_fee: f64,
    pub uniform_fee: f64,
    pub lab_fee: f64,
    pub computer_fee: f64,
    pub sports_fee: f64,
    pub activity_fee: f64,
    pub maintenance_fee: f64,
    pub bus_fee: f64,
    pub previous_balance: f64,
    pub extra_fees: f64,
    pub discount: f64,
    pub scholarship: f64,
    pub total_fee: f64,
    pub amount_paid: f64,
    pub balance: f64,
    pub payment_status: String,
    pub last_payment_date: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Payment {
    pub id: i64,
    pub student_id: String,
    pub student_name: Option<String>,
    pub bill_id: Option<i64>,
    pub payment_date: String,
    pub amount: f64,
    pub payment_mode: String,
    pub receipt_number: String,
    pub academic_year_id: Option<i64>,
    pub notes: Option<String>,
    pub allocated_admission: Option<f64>,
    pub allocated_other: Option<f64>,
    pub allocated_tuition: Option<f64>,
    pub allocated_bus: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SyncQueueItem {
    pub id: i64,
    pub student_id: String,
    pub status: String,
    pub retry_count: i64,
    pub last_attempt_at: Option<String>,
    pub created_at: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DashboardStats {
    pub todays_collection: f64,
    pub monthly_collection: f64,
    pub pending_fees: f64,
    pub pending_sync: i64,
    pub last_sync_time: String,
    pub active_academic_year: String,
    pub total_students: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GenerateBillRequest {
    pub student_id: String,
    pub academic_year_id: i64,
    pub tuition_fee: f64,
    pub admission_fee: f64,
    pub exam_fee: f64,
    pub book_fee: f64,
    pub uniform_fee: f64,
    pub lab_fee: f64,
    pub computer_fee: f64,
    pub sports_fee: f64,
    pub activity_fee: f64,
    pub maintenance_fee: f64,
    pub bus_fee: f64,
    pub previous_balance: f64,
    pub extra_fees: f64,
    pub discount: f64,
    pub scholarship: f64,
    pub total_fee: f64,
    pub extra_fee_items: Vec<ExtraFeeItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExtraFeeItem {
    pub name: String,
    pub amount: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RecordPaymentRequest {
    pub student_id: String,
    pub bill_id: i64,
    pub amount: f64,
    pub payment_mode: String,
    pub payment_date: String,
    pub notes: Option<String>,
    pub allocated_admission: Option<f64>,
    pub allocated_other: Option<f64>,
    pub allocated_tuition: Option<f64>,
    pub allocated_bus: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReceiptData {
    pub receipt_number: String,
    pub student: Student,
    pub payment: Payment,
    pub bill: Bill,
    pub school: SchoolSettings,
    pub academic_year_name: String,
    pub bill_items: Vec<ExtraFeeItem>,
}

// Struct representing class rows in Supabase
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SupabaseClass {
    pub id: String,
    pub name: String,
    pub section: Option<String>,
    pub grade: Option<i32>,
}

// Struct representing student rows in Supabase
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SupabaseStudent {
    pub id: String,
    pub roll_no: Option<String>, // nullable in Supabase
    pub name: String,
    pub class_id: Option<String>,
    pub r#type: Option<String>, // 'type' is a reserved keyword in Rust
    pub phone: Option<String>,
    pub parent_name: Option<String>,
    pub academic_year: Option<String>,
}

// ── MONTHLY TUITION & BUS FEE MODELS ────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AcademicYearMonths {
    pub academic_year_id: i64,
    pub start_month: u32,
    pub end_month: u32,
    pub start_year: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MonthlyTuitionItem {
    pub id: i64,
    pub student_id: String,
    pub academic_year_id: i64,
    pub month: u32,
    pub year: i32,
    pub month_name: String,
    pub monthly_amount: f64,
    pub amount_paid: f64,
    pub balance: f64,
    pub status: String,  // paid | partial | unpaid
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MonthlyBusUsageItem {
    pub id: i64,
    pub student_id: String,
    pub student_name: Option<String>,
    pub academic_year_id: i64,
    pub month: u32,
    pub year: i32,
    pub month_name: String,
    pub bus_used: bool,
    pub bus_fee: f64,
    pub amount_paid: f64,
    pub balance: f64,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MonthlyBillSummary {
    pub student_id: String,
    pub student_name: String,
    pub academic_year_id: i64,
    pub current_month: u32,
    pub current_year: i32,
    pub current_month_tuition: f64,
    pub outstanding_tuition: f64,   // sum of unpaid previous months tuition
    pub outstanding_bus: f64,       // sum of unpaid previous bus fees
    pub current_bus_fee: f64,       // this month's bus fee (0 if not using bus)
    pub months_overdue: Vec<MonthlyTuitionItem>,
    pub bus_months_overdue: Vec<MonthlyBusUsageItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BusReportItem {
    pub student_id: String,
    pub student_name: String,
    pub month: u32,
    pub year: i32,
    pub month_name: String,
    pub bus_used: bool,
    pub bus_fee: f64,
    pub amount_paid: f64,
    pub balance: f64,
    pub status: String,
}


