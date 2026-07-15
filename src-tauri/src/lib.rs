mod db;
mod models;
mod sync;
mod commands;
mod monthly;

use tauri::Manager;
use log::info;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            commands::login,
            commands::get_school_settings,
            commands::update_school_settings,
            commands::get_academic_years,
            commands::create_academic_year,
            commands::set_active_academic_year,
            commands::get_classes,
            commands::create_class,
            commands::update_class,
            commands::delete_class,
            commands::get_students,
            commands::get_student,
            commands::create_student,
            commands::update_student,
            commands::get_bus_stops,
            commands::create_bus_stop,
            commands::update_bus_stop,
            commands::delete_bus_stop,
            commands::get_dashboard_stats,
            commands::generate_bill,
            commands::get_bill,
            commands::record_payment,
            commands::get_payments,
            commands::revert_payment,
            commands::get_receipt_data,
            commands::get_bill_items,
            commands::get_settings,
            commands::update_setting,
            commands::sync_students_classes,
            commands::sync_fees,
            commands::get_sync_queue,
            commands::clear_sync_logs,
            commands::retry_failed_sync,
            commands::get_reports,
            commands::search_students,
            commands::promote_students,
            // Monthly Tuition & Bus Commands
            commands::configure_year_months,
            commands::get_year_months,
            commands::generate_monthly_tuition_all,
            commands::generate_monthly_tuition_for_student,
            commands::get_monthly_tuition_status,
            commands::get_outstanding_tuition,
            commands::get_monthly_bill_summary,
            commands::set_monthly_bus_usage,
            commands::get_student_bus_usage,
            commands::record_monthly_payment,
            commands::get_bus_report,
            commands::open_external_url,
        ])
        .setup(|app| {
            let app_dir = app.path().app_data_dir()
                .expect("Failed to get app data dir");
            std::fs::create_dir_all(&app_dir)?;
            let app_dir_str = app_dir.to_str().unwrap().to_string();
            db::init_db(&app_dir_str).expect("Failed to initialize DB");
            info!("MSS Billing started. Data dir: {}", app_dir_str);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

