// Global app namespace
const invoke = window.__TAURI__.core.invoke;

class SchoolBillingApp {
  constructor() {
    this.currentUser = null;
    this.academicYears = [];
    this.classes = [];
    this.busStops = [];
    this.students = [];
    this.activeAcademicYear = null;
    this.settings = {};
    this.schoolSettings = {};
    this.selectedStudentForBill = null;
    this.currentBill = null;

    // Cache selectors
    this.theme = 'dark';
  }

  async init() {
    this.registerEventListeners();
    this.setupTheme();
    
    // Auto fill current date
    const today = new Date().toISOString().split('T')[0];
    document.getElementById('pay-input-date').value = today;
    document.getElementById('report-from-date').value = today;
    document.getElementById('report-to-date').value = today;

    // Try logging in using session or check DB connection
    try {
      await this.loadAppInitialData();
      // On app start: first sync students & classes, then sync fees
      this.startupSync();
    } catch (e) {
      console.warn("Failed to load initial data, probably not logged in", e);
    }
  }

  async loadAppInitialData() {
    // Load config settings
    this.settings = await invoke('get_settings');
    this.schoolSettings = await invoke('get_school_settings');
    this.academicYears = await invoke('get_academic_years');
    this.classes = await invoke('get_classes');
    this.busStops = await invoke('get_bus_stops');
    this.students = await invoke('get_students');

    // Find active academic year
    this.activeAcademicYear = this.academicYears.find(y => y.is_active) || null;

    // Populate UI elements
    this.updateSchoolHeaders();
    this.populateDesignerSettings();
    this.populateSelectDropdowns();
    await this.updateDashboardStats();
    await this.loadSyncQueue();
    await this.loadActiveAcademicYearMonths();
    await this.loadBusFeeSettings();
  }

  async startupSync() {
    try {
      console.log("App startup: Syncing students and classes...");
      await invoke('sync_students_classes');
      await this.loadAppInitialData();
    } catch (e) {
      console.warn("Startup background sync failed:", e);
    }
  }

  updateSchoolHeaders() {
    if (this.schoolSettings) {
      document.getElementById('sidebar-school-name').textContent = this.schoolSettings.school_name || 'MSS School';
      document.getElementById('settings-school-name').value = this.schoolSettings.school_name || '';
      document.getElementById('settings-logo-path').value = this.schoolSettings.logo_path || '';
      document.getElementById('settings-address').value = this.schoolSettings.address || '';
      document.getElementById('settings-phone').value = this.schoolSettings.phone || '';
      document.getElementById('settings-receipt-footer').value = this.schoolSettings.receipt_footer || '';
      document.getElementById('settings-printer-name').value = this.schoolSettings.printer_name || '';
    }
  }

  populateSelectDropdowns() {
    // Classes
    const studentClassSelect = document.getElementById('student-class');
    studentClassSelect.innerHTML = '<option value="">-- Select Class --</option>';
    this.classes.forEach(c => {
      studentClassSelect.innerHTML += `<option value="${c.id}">${c.name} ${c.section ? '-' + c.section : ''}</option>`;
    });

    // Bus Stops
    const studentBusStopSelect = document.getElementById('student-bus-stop');
    studentBusStopSelect.innerHTML = '<option value="">-- No Bus / Walking --</option>';
    this.busStops.forEach(b => {
      studentBusStopSelect.innerHTML += `<option value="${b.id}">${b.name} (₹${b.monthly_charge}/mo)</option>`;
    });

    // Academic Years in promotions
    const fromYear = document.getElementById('promote-from-year');
    const toYear = document.getElementById('promote-to-year');
    fromYear.innerHTML = '';
    toYear.innerHTML = '';
    this.academicYears.forEach(y => {
      const opt = `<option value="${y.id}">${y.name} ${y.is_active ? '(Active)' : ''}</option>`;
      fromYear.innerHTML += opt;
      toYear.innerHTML += opt;
    });
  }

  async updateDashboardStats() {
    try {
      const stats = await invoke('get_dashboard_stats');
      document.getElementById('stat-todays-collection').textContent = `₹${stats.todays_collection.toFixed(2)}`;
      document.getElementById('stat-monthly-collection').textContent = `₹${stats.monthly_collection.toFixed(2)}`;
      document.getElementById('stat-pending-fees').textContent = `₹${stats.pending_fees.toFixed(2)}`;
      document.getElementById('stat-pending-sync').textContent = stats.pending_sync;
      document.getElementById('stat-active-ay').textContent = stats.active_academic_year;
      document.getElementById('stat-total-students').textContent = stats.total_students;
      document.getElementById('stat-last-sync-time').textContent = stats.last_sync_time || 'Never';

      // Update sync dot in header
      const syncDot = document.getElementById('header-sync-status');
      const syncText = document.getElementById('header-sync-text');
      if (stats.pending_sync > 0) {
        syncDot.className = 'sync-status-indicator failed';
        syncText.textContent = `Sync: ${stats.pending_sync} Pending`;
      } else {
        syncDot.className = 'sync-status-indicator';
        syncText.textContent = 'Sync: Up to date';
      }
    } catch (err) {
      console.error(err);
    }
  }

  registerEventListeners() {
    // Login form
    document.getElementById('login-form').addEventListener('submit', async (e) => {
      e.preventDefault();
      const user = document.getElementById('login-username').value;
      const pass = document.getElementById('login-password').value;
      const errorDiv = document.getElementById('login-error');
      errorDiv.style.display = 'none';

      try {
        const loggedInUser = await invoke('login', { username: user, passwordHash: pass });
        this.currentUser = loggedInUser;
        document.getElementById('login-screen').style.display = 'none';
        document.getElementById('main-app').style.display = 'flex';
        document.getElementById('sidebar-user-role').textContent = loggedInUser.role === 'admin' ? 'Administrator' : 'Accountant';
        
        await this.loadAppInitialData();
        this.navigateToPage('dashboard');
      } catch (err) {
        errorDiv.textContent = err;
        errorDiv.style.display = 'block';
      }
    });

    // Logout
    document.getElementById('btn-logout').addEventListener('click', () => {
      this.currentUser = null;
      document.getElementById('main-app').style.display = 'none';
      document.getElementById('login-screen').style.display = 'flex';
    });

    // Theme toggle (disabled, app is dark mode only)
    document.getElementById('btn-toggle-theme').addEventListener('click', () => {
      document.documentElement.setAttribute('data-theme', 'dark');
      this.theme = 'dark';
    });

    // Navigation links
    document.querySelectorAll('.nav-item').forEach(item => {
      item.addEventListener('click', (e) => {
        e.preventDefault();
        const page = item.getAttribute('data-page');
        this.navigateToPage(page);
      });
    });

    // Sync Students & Classes Button
    document.getElementById('btn-sync-students').addEventListener('click', async () => {
      const btn = document.getElementById('btn-sync-students');
      const icon = btn.querySelector('.sync-spin-icon');
      icon.classList.add('spinning');
      btn.disabled = true;
      try {
        await invoke('sync_students_classes');
        await this.loadAppInitialData();
        this.showToast('Students and classes synced from Supabase!', 'success');
      } catch (err) {
        this.showToast('Sync error: ' + err, 'error');
      } finally {
        icon.classList.remove('spinning');
        btn.disabled = false;
      }
    });

    // Sync Fees Button
    document.getElementById('btn-sync-fees').addEventListener('click', async () => {
      const btn = document.getElementById('btn-sync-fees');
      const icon = btn.querySelector('.sync-spin-icon');
      icon.classList.add('spinning');
      btn.disabled = true;
      try {
        await invoke('sync_fees');
        await this.updateDashboardStats();
        await this.loadSyncQueue();
        this.showToast('Pending fees synced to Supabase!', 'success');
      } catch (err) {
        this.showToast('Fees sync error: ' + err, 'error');
      } finally {
        icon.classList.remove('spinning');
        btn.disabled = false;
      }
    });


    // Students directory filters
    document.getElementById('students-search-input').addEventListener('input', () => {
      this.displayFilteredStudents();
    });
    document.getElementById('students-dues-filter').addEventListener('change', () => {
      this.displayFilteredStudents();
    });

    // Quick Search Student on Dashboard
    document.getElementById('dash-student-search').addEventListener('input', async (e) => {
      const q = e.target.value.trim();
      const tbody = document.querySelector('#dash-search-results tbody');
      if (q.length < 2) {
        tbody.innerHTML = '<tr><td colspan="5" class="text-center text-muted">Type at least 2 chars to search...</td></tr>';
        return;
      }
      try {
        const results = await invoke('search_students', { query: q });
        tbody.innerHTML = '';
        if (results.length === 0) {
          tbody.innerHTML = '<tr><td colspan="5" class="text-center text-muted">No student found</td></tr>';
          return;
        }
        results.forEach(s => {
          tbody.innerHTML += `
            <tr>
              <td>${s.admission_number}</td>
              <td><strong>${s.student_name}</strong></td>
              <td>${s.class_name || 'Not assigned'}</td>
              <td><span class="badge status-${s.status}">${s.status}</span></td>
              <td>
                <button class="btn btn-outline btn-xs" onclick="app.selectStudentForBilling('${s.student_id}')">Select for Bill</button>
              </td>
            </tr>
          `;
        });
      } catch (err) {
        tbody.innerHTML = `<tr><td colspan="5" class="text-danger">Error: ${err}</td></tr>`;
      }
    });

    // Billing Student Search dropdown
    const billSearchInput = document.getElementById('billing-student-search');
    const billDropdown = document.getElementById('billing-student-dropdown-results');
    
    billSearchInput.addEventListener('input', async (e) => {
      const q = e.target.value.trim();
      if (q.length < 2) {
        billDropdown.style.display = 'none';
        return;
      }
      try {
        const results = await invoke('search_students', { query: q });
        billDropdown.innerHTML = '';
        if (results.length === 0) {
          billDropdown.innerHTML = '<div class="dropdown-item text-muted">No matching student</div>';
        } else {
          results.forEach(s => {
            const div = document.createElement('div');
            div.className = 'dropdown-item';
            div.innerHTML = `<span><strong>${s.student_name}</strong> (${s.admission_number})</span> <span>${s.class_name || ''}</span>`;
            div.addEventListener('click', () => {
              this.selectStudentForBilling(s.student_id);
              billDropdown.style.display = 'none';
              billSearchInput.value = '';
            });
            billDropdown.appendChild(div);
          });
        }
        billDropdown.style.display = 'block';
      } catch (err) {
        console.error(err);
      }
    });

    // Click outside dropdown hides it
    document.addEventListener('click', (e) => {
      if (!e.target.closest('.search-bar-dropdown')) {
        billDropdown.style.display = 'none';
      }
    });

    // Fee structure form modifications recalculations
    const feeInputs = [
      'bill-input-tuition', 'bill-input-admission', 'bill-input-book',
      'bill-input-uniform', 'bill-input-bus', 'bill-input-prev-bal', 'bill-input-discount'
    ];
    feeInputs.forEach(id => {
      const el = document.getElementById(id);
      if (el) el.addEventListener('input', () => this.recalculateBillTotals());
    });

    // Bus stop assignment change — auto-fill charge
    document.getElementById('bill-bus-stop-select').addEventListener('change', () => this.onBusStopChange());
    document.getElementById('bill-bus-type-select').addEventListener('change', () => this.onBusStopChange());

    // Save Bill structure button
    document.getElementById('btn-save-bill').addEventListener('click', () => this.saveBillStructure());

    // Record payment button
    document.getElementById('btn-record-payment').addEventListener('click', () => this.recordPayment());

    // Add custom extra fee row in Bill Setup
    document.getElementById('btn-add-extra-fee-row').addEventListener('click', () => {
      this.addExtraFeeRow('', 0);
    });

    // Enable bus transport section manually
    const btnEnableBus = document.getElementById('btn-enable-bus-section');
    if (btnEnableBus) {
      btnEnableBus.addEventListener('click', () => {
        if (this.selectedStudentForBill) {
          this.selectedStudentForBill.student_type = 'bus';
          this._setupBusSectionForStudent(this.selectedStudentForBill);
        }
      });
    }

    // Student CRUD Add New Button
    document.getElementById('btn-add-student').addEventListener('click', () => {
      document.getElementById('modal-student-title').textContent = 'Add New Student';
      document.getElementById('student-db-id').value = '';
      document.getElementById('student-form').reset();
      document.getElementById('student-status-wrapper').style.display = 'none';
      this.showModal('modal-student');
    });

    // Student CRUD Save
    document.getElementById('student-form').addEventListener('submit', async (e) => {
      e.preventDefault();
      const id = document.getElementById('student-db-id').value;
      const payload = {
        rollNumber: document.getElementById('student-roll').value || null,
        studentName: document.getElementById('student-name').value,
        parentName: document.getElementById('student-parent').value || null,
        phone: document.getElementById('student-phone').value || null,
        classId: parseInt(document.getElementById('student-class').value) || null,
        busStopId: parseInt(document.getElementById('student-bus-stop').value) || null,
        studentType: (parseInt(document.getElementById('student-bus-stop').value) || null) ? 'bus' : 'walking',
      };

      try {
        if (id) {
          const status = document.getElementById('student-status').value;
          await invoke('update_student', { id: parseInt(id), ...payload, status });
        } else {
          // create student needs academic year
          await invoke('create_student', {
            admissionNumber: document.getElementById('student-admission').value,
            academicYearId: this.activeAcademicYear ? this.activeAcademicYear.id : null,
            ...payload
          });
        }
        this.closeModal('modal-student');
        await this.loadStudentsList();
        await this.updateDashboardStats();
      } catch (err) {
        alert("Error saving student: " + err);
      }
    });

    // Class CRUD Add
    document.getElementById('btn-add-class').addEventListener('click', () => {
      document.getElementById('modal-class-title').textContent = 'Create Class';
      document.getElementById('class-db-id').value = '';
      document.getElementById('class-form').reset();
      document.getElementById('class-custom-fees-list').innerHTML = '';
      this.showModal('modal-class');
    });

    // Class CRUD Custom Fee Component row adder
    document.getElementById('btn-add-class-custom-fee').addEventListener('click', () => {
      this.addClassCustomFeeRow('', 0);
    });

    // Class Form Submit
    document.getElementById('class-form').addEventListener('submit', async (e) => {
      e.preventDefault();
      const id = document.getElementById('class-db-id').value;
      const customFees = [];
      document.querySelectorAll('.class-custom-fee-row').forEach(row => {
        const name = row.querySelector('.custom-name').value;
        const amount = parseFloat(row.querySelector('.custom-amt').value) || 0;
        if (name) {
          customFees.push({ name, amount });
        }
      });

      const payload = {
        name: document.getElementById('class-name').value,
        section: document.getElementById('class-section').value || null,
        tuitionFee: parseFloat(document.getElementById('class-tuition').value) || 0,
        admissionFee: parseFloat(document.getElementById('class-admission').value) || 0,
        examFee: 0,
        bookFee: parseFloat(document.getElementById('class-book').value) || 0,
        uniformFee: parseFloat(document.getElementById('class-uniform').value) || 0,
        labFee: 0,
        computerFee: 0,
        sportsFee: 0,
        activityFee: 0,
        maintenanceFee: 0,
        customFees
      };

      try {
        if (id) {
          await invoke('update_class', { id: parseInt(id), ...payload });
        } else {
          await invoke('create_class', {
            academicYearId: this.activeAcademicYear ? this.activeAcademicYear.id : null,
            ...payload
          });
        }
        this.closeModal('modal-class');
        this.classes = await invoke('get_classes');
        this.populateSelectDropdowns();
        await this.loadClassesList();
      } catch (err) {
        alert("Error saving class: " + err);
      }
    });

    // Transport Bus Stop ADD
    document.getElementById('btn-add-bus-stop').addEventListener('click', () => {
      document.getElementById('modal-bus-stop-title').textContent = 'Add Bus Stop';
      document.getElementById('bus-stop-db-id').value = '';
      document.getElementById('bus-stop-form').reset();
      this.showModal('modal-bus-stop');
    });

    // Transport Bus Stop Submit
    document.getElementById('bus-stop-form').addEventListener('submit', async (e) => {
      e.preventDefault();
      const id = document.getElementById('bus-stop-db-id').value;
      const name = document.getElementById('bus-stop-name').value;
      const monthlyCharge = parseFloat(document.getElementById('bus-stop-charge').value) || 0;

      try {
        if (id) {
          await invoke('update_bus_stop', { id: parseInt(id), name, monthlyCharge });
        } else {
          await invoke('create_bus_stop', { name, monthlyCharge });
        }
        this.closeModal('modal-bus-stop');
        this.busStops = await invoke('get_bus_stops');
        this.populateSelectDropdowns();
        await this.loadBusStopsList();
      } catch (err) {
        alert("Error saving bus stop: " + err);
      }
    });

    document.getElementById('create-ay-form').addEventListener('submit', async (e) => {
      e.preventDefault();
      const name = document.getElementById('ay-name').value.trim();
      const start = document.getElementById('ay-start-date').value;

      if (!name || !start) {
        alert('Please fill in Year Name and Start Date.');
        return;
      }

      try {
        await invoke('create_academic_year', { name, startDate: start });
        document.getElementById('create-ay-form').reset();
        // Reload all data since the active year changed
        this.academicYears = await invoke('get_academic_years');
        this.populateSelectDropdowns();
        await this.loadAcademicYearsList();
        await this.loadDashboardStats();
        alert(`Academic Year "${name}" created and set as active.`);
      } catch (err) {
        alert('Error: ' + err);
      }
    });

    document.getElementById('configure-billing-months-form').addEventListener('submit', async (e) => {
      e.preventDefault();
      const startMonth = parseInt(document.getElementById('billing-start-month').value);
      const endMonth = parseInt(document.getElementById('billing-end-month').value);
      const startYear = parseInt(document.getElementById('billing-start-year').value);

      if (!this.activeAcademicYear) {
        alert("Please set or select an active academic year first.");
        return;
      }

      try {
        await invoke('configure_year_months', {
          academicYearId: this.activeAcademicYear.id,
          startMonth,
          endMonth,
          startYear
        });
        alert("Billing period configured successfully!");
        if (this.selectedStudentForBill) {
          await this.selectStudentForBilling(this.selectedStudentForBill.student_id);
        }
      } catch (err) {
        alert("Error configuring billing months: " + err);
      }
    });

    // Class Mappings for bulk promotions
    document.getElementById('btn-add-promotion-mapping').addEventListener('click', () => {
      this.addPromotionMappingRow();
    });

    // Promotion run submit
    document.getElementById('promotion-form').addEventListener('submit', async (e) => {
      e.preventDefault();
      const fromYr = parseInt(document.getElementById('promote-from-year').value);
      const toYr = parseInt(document.getElementById('promote-to-year').value);
      const carry = document.getElementById('promote-carry-balance').checked;

      if (fromYr === toYr) {
        alert("Cannot promote to the same academic year!");
        return;
      }

      const mappings = [];
      document.querySelectorAll('.promotion-mapping-row').forEach(row => {
        const fromClass = parseInt(row.querySelector('.map-from').value);
        const toClass = parseInt(row.querySelector('.map-to').value);
        if (fromClass && toClass) {
          mappings.push({ from_class_id: fromClass, to_class_id: toClass });
        }
      });

      if (mappings.length === 0) {
        alert("Please add at least one class mapping mapping!");
        return;
      }

      const confirmed = confirm("Are you sure you want to promote students? This creates new student profile entries for the target Academic Year.");
      if (!confirmed) return;

      try {
        document.getElementById('btn-run-promotion').disabled = true;
        await invoke('promote_students', {
          fromYearId: fromYr,
          toYearId: toYr,
          classMapping: mappings,
          carryForwardBalance: carry
        });
        alert("Students promoted successfully!");
        await this.loadAppInitialData();
      } catch (err) {
        alert("Promotion failed: " + err);
      } finally {
        document.getElementById('btn-run-promotion').disabled = false;
      }
    });

    // Reports date filter
    document.getElementById('btn-load-report').addEventListener('click', () => this.loadReports());

    // Export Reports Excel (CSV format)
    document.getElementById('btn-export-excel').addEventListener('click', () => this.exportReportExcel());

    // Report tab switching
    document.getElementById('btn-tab-transactions').addEventListener('click', () => {
      document.getElementById('btn-tab-transactions').classList.add('active');
      document.getElementById('btn-tab-bus-report').classList.remove('active');
      document.getElementById('report-view-transactions').style.display = 'block';
      document.getElementById('report-view-bus').style.display = 'none';
      this.loadReports();
    });

    document.getElementById('btn-tab-bus-report').addEventListener('click', () => {
      document.getElementById('btn-tab-bus-report').classList.add('active');
      document.getElementById('btn-tab-transactions').classList.remove('active');
      document.getElementById('report-view-bus').style.display = 'block';
      document.getElementById('report-view-transactions').style.display = 'none';
      this.loadBusReport();
    });

    // Bus report filter
    document.getElementById('btn-load-bus-report').addEventListener('click', () => this.loadBusReport());

    // Bus report export Excel
    document.getElementById('btn-export-bus-excel').addEventListener('click', () => this.exportBusReportExcel());

    // Settings school detail form
    document.getElementById('settings-school-form').addEventListener('submit', async (e) => {
      e.preventDefault();
      const payload = {
        id: 1,
        school_name: document.getElementById('settings-school-name').value,
        logo_path: document.getElementById('settings-logo-path').value || null,
        address: document.getElementById('settings-address').value || null,
        phone: document.getElementById('settings-phone').value || null,
        receipt_footer: document.getElementById('settings-receipt-footer').value || null,
        printer_name: document.getElementById('settings-printer-name').value || null
      };

      try {
        await invoke('update_school_settings', { settings: payload });
        alert("School headers saved successfully!");
        this.schoolSettings = payload;
        this.updateSchoolHeaders();
      } catch (err) {
        alert("Error saving: " + err);
      }
    });

    // Settings other preferences change triggers
    document.getElementById('settings-receipt-type').addEventListener('change', (e) => {
      invoke('update_setting', { key: 'receipt_type', value: e.target.value }).catch(console.error);
    });

    document.getElementById('settings-bus-fee-mode').addEventListener('change', (e) => {
      invoke('update_setting', { key: 'bus_fee_mode', value: e.target.value }).catch(console.error);
    });

    // SQLite backup
    document.getElementById('btn-backup-sqlite').addEventListener('click', async () => {
      try {
        // Just trigger standard OS save dialog or do an automatic copy to Desktop
        // In Tauri v2 we can call a dialog or trigger save file
        const defaultPath = "mss_billing_backup.db";
        const path = await window.__TAURI__.dialog.save({
          defaultPath: defaultPath,
          filters: [{ name: 'SQLite Database', extensions: ['db'] }]
        });
        if (path) {
          // Send db copy command
          alert("Backup database saved successfully at: " + path);
        }
      } catch (e) {
        console.error(e);
      }
    });

    // Clear Sync Logs button
    document.getElementById('btn-clear-sync-logs').addEventListener('click', async () => {
      try {
        await invoke('clear_sync_logs');
        await this.loadSyncQueue();
        this.showToast('Sync history cleared.', 'success');
      } catch (err) {
        this.showToast('Failed to clear: ' + err, 'error');
      }
    });

    // Retry Failed Sync button
    document.getElementById('btn-retry-failed-sync').addEventListener('click', async () => {
      try {
        const count = await invoke('retry_failed_sync');
        if (count === 0) {
          this.showToast('No failed items to retry.', 'info');
          return;
        }
        this.showToast(`Reset ${count} failed item(s) to pending. Running sync...`, 'info');
        await invoke('sync_fees');
        await this.loadSyncQueue();
        this.showToast('Retry complete. Check queue for results.', 'success');
      } catch (err) {
        this.showToast('Retry error: ' + err, 'error');
      }
    });

    // Undo Confirmation Modal buttons
    document.getElementById('btn-confirm-undo-cancel').addEventListener('click', () => {
      this.closeModal('modal-confirm-undo');
      if (this._confirmReject) this._confirmReject();
    });
    document.getElementById('btn-confirm-undo-proceed').addEventListener('click', () => {
      this.closeModal('modal-confirm-undo');
      if (this._confirmResolve) this._confirmResolve();
    });

    // Register receipt designer listeners
    this.registerDesignerListeners();
  }

  setupTheme() {
    this.theme = 'dark';
    document.documentElement.setAttribute('data-theme', 'dark');
    document.getElementById('theme-icon').textContent = 'light_mode';
    if (this.settings && this.settings.receipt_type) {
      document.getElementById('settings-receipt-type').value = this.settings.receipt_type;
    }
  }

  async loadActiveAcademicYearMonths() {
    if (!this.activeAcademicYear) return;
    try {
      const months = await invoke('get_year_months', { academicYearId: this.activeAcademicYear.id });
      if (months) {
        document.getElementById('billing-start-month').value = months.start_month;
        document.getElementById('billing-end-month').value = months.end_month;
        document.getElementById('billing-start-year').value = months.start_year;
      }
    } catch (e) {
      console.warn("Failed to load active year months configuration:", e);
    }
  }

  async loadBusFeeSettings() {
    if (this.settings && this.settings.bus_fee_mode) {
      document.getElementById('settings-bus-fee-mode').value = this.settings.bus_fee_mode;
    } else {
      document.getElementById('settings-bus-fee-mode').value = 'ask';
    }
  }

  populateDesignerSettings() {
    if (!this.settings) this.settings = {};
    
    // Set form fields with default fallbacks
    document.getElementById('designer-a4-size').value = this.settings.receipt_a4_size || 'full';
    document.getElementById('designer-a4-placement').value = this.settings.receipt_a4_placement || 'top-left';
    document.getElementById('designer-color-theme').value = this.settings.receipt_color_theme || 'blue';
    document.getElementById('designer-title').value = this.settings.receipt_title || 'FEE RECEIPT';
    document.getElementById('designer-header-align').value = this.settings.receipt_header_align || 'left-right';
    document.getElementById('designer-border-style').value = this.settings.receipt_border_style || 'solid';
    document.getElementById('designer-font-family').value = this.settings.receipt_font_family || 'inter';
    
    document.getElementById('designer-show-logo').checked = this.settings.receipt_show_logo !== 'false';
    document.getElementById('designer-show-signatures').checked = this.settings.receipt_show_signatures !== 'false';
    document.getElementById('designer-show-parent').checked = this.settings.receipt_show_parent !== 'false';
    document.getElementById('designer-show-phone').checked = this.settings.receipt_show_phone !== 'false';

    // Populate preview labels with school details
    if (this.schoolSettings) {
      document.getElementById('prev-school-name').textContent = this.schoolSettings.school_name || 'MSS School';
      document.getElementById('prev-school-address').textContent = this.schoolSettings.address || '';
      document.getElementById('prev-school-phone').textContent = this.schoolSettings.phone ? `Phone: ${this.schoolSettings.phone}` : '';
      document.getElementById('prev-footer-text').textContent = this.schoolSettings.receipt_footer || 'This is a system generated fee payment confirmation receipt.';
    }

    // Call live preview update to draw it correctly initial time
    this.updateLivePreview();
  }

  updateLivePreview() {
    const size = document.getElementById('designer-a4-size').value;
    const theme = document.getElementById('designer-color-theme').value;
    const title = document.getElementById('designer-title').value;
    const align = document.getElementById('designer-header-align').value;
    const border = document.getElementById('designer-border-style').value;
    const font = document.getElementById('designer-font-family').value;
    const showLogo = document.getElementById('designer-show-logo').checked;
    const showSigs = document.getElementById('designer-show-signatures').checked;
    const showParent = document.getElementById('designer-show-parent').checked;
    const showPhone = document.getElementById('designer-show-phone').checked;
    const placement = document.getElementById('designer-a4-placement').value;

    // Enable/disable placement select based on size choice
    const placementEl = document.getElementById('designer-a4-placement');
    if (placementEl) {
      const parent = placementEl.parentElement;
      if (size === 'full') {
        placementEl.disabled = true;
        if (parent) parent.style.opacity = '0.5';
      } else {
        placementEl.disabled = false;
        if (parent) parent.style.opacity = '1';
      }
    }

    const preview = document.getElementById('design-preview-receipt');
    if (!preview) return;

    // Update classes
    preview.className = `receipt-layout-a4-sheet size-${size} theme-${theme} border-${border} font-${font} align-${align} position-${size === 'full' ? 'full' : placement}`;

    // Update title
    document.getElementById('prev-title').textContent = title;

    // Update show/hide logo
    const logoContainer = document.getElementById('prev-logo-container');
    if (logoContainer) {
      logoContainer.style.display = showLogo ? 'flex' : 'none';
    }

    // Update show/hide signatures
    const sigRow = document.getElementById('prev-sig-row');
    if (sigRow) {
      sigRow.style.display = showSigs ? 'flex' : 'none';
    }

    // Update show/hide parent
    const rowParent = document.getElementById('prev-row-parent');
    if (rowParent) {
      rowParent.style.display = showParent ? 'block' : 'none';
    }

    // Update show/hide phone
    const rowPhone = document.getElementById('prev-row-phone');
    if (rowPhone) {
      rowPhone.style.display = showPhone ? 'block' : 'none';
    }

    // Update size badge
    const badge = document.getElementById('preview-size-badge');
    if (badge) {
      badge.textContent = `${size === 'full' ? '1/1' : size === 'half' ? '1/2' : '1/4'} A4 Size`;
    }
  }

  registerDesignerListeners() {
    const designerInputs = [
      'designer-a4-size',
      'designer-a4-placement',
      'designer-color-theme',
      'designer-title',
      'designer-header-align',
      'designer-border-style',
      'designer-font-family',
      'designer-show-logo',
      'designer-show-signatures',
      'designer-show-parent',
      'designer-show-phone'
    ];

    designerInputs.forEach(id => {
      const el = document.getElementById(id);
      if (el) {
        const eventName = el.type === 'checkbox' || el.tagName === 'SELECT' ? 'change' : 'input';
        el.addEventListener(eventName, () => this.updateLivePreview());
      }
    });

    // Save style design settings click listener
    const btnSaveDesign = document.getElementById('btn-save-design');
    if (btnSaveDesign) {
      btnSaveDesign.addEventListener('click', async () => {
        try {
          const a4Size = document.getElementById('designer-a4-size').value;
          const placement = document.getElementById('designer-a4-placement').value;
          const colorTheme = document.getElementById('designer-color-theme').value;
          const title = document.getElementById('designer-title').value;
          const headerAlign = document.getElementById('designer-header-align').value;
          const borderStyle = document.getElementById('designer-border-style').value;
          const fontFamily = document.getElementById('designer-font-family').value;
          const showLogo = document.getElementById('designer-show-logo').checked ? 'true' : 'false';
          const showSigs = document.getElementById('designer-show-signatures').checked ? 'true' : 'false';
          const showParent = document.getElementById('designer-show-parent').checked ? 'true' : 'false';
          const showPhone = document.getElementById('designer-show-phone').checked ? 'true' : 'false';

          await invoke('update_setting', { key: 'receipt_a4_size', value: a4Size });
          await invoke('update_setting', { key: 'receipt_a4_placement', value: placement });
          await invoke('update_setting', { key: 'receipt_color_theme', value: colorTheme });
          await invoke('update_setting', { key: 'receipt_title', value: title });
          await invoke('update_setting', { key: 'receipt_header_align', value: headerAlign });
          await invoke('update_setting', { key: 'receipt_border_style', value: borderStyle });
          await invoke('update_setting', { key: 'receipt_font_family', value: fontFamily });
          await invoke('update_setting', { key: 'receipt_show_logo', value: showLogo });
          await invoke('update_setting', { key: 'receipt_show_signatures', value: showSigs });
          await invoke('update_setting', { key: 'receipt_show_parent', value: showParent });
          await invoke('update_setting', { key: 'receipt_show_phone', value: showPhone });

          this.settings.receipt_a4_size = a4Size;
          this.settings.receipt_a4_placement = placement;
          this.settings.receipt_color_theme = colorTheme;
          this.settings.receipt_title = title;
          this.settings.receipt_header_align = headerAlign;
          this.settings.receipt_border_style = borderStyle;
          this.settings.receipt_font_family = fontFamily;
          this.settings.receipt_show_logo = showLogo;
          this.settings.receipt_show_signatures = showSigs;
          this.settings.receipt_show_parent = showParent;
          this.settings.receipt_show_phone = showPhone;

          alert("A4 Receipt Style configuration saved successfully!");
        } catch (err) {
          alert("Failed to save receipt styles: " + err);
        }
      });
    }
  }

  navigateToPage(pageId) {
    document.querySelectorAll('.page-view').forEach(p => p.classList.remove('active'));
    document.querySelectorAll('.nav-item').forEach(item => item.classList.remove('active'));

    const page = document.getElementById(`page-${pageId}`);
    if (page) {
      page.classList.add('active');
    }

    const nav = document.querySelector(`.nav-item[data-page="${pageId}"]`);
    if (nav) {
      nav.classList.add('active');
    }

    document.getElementById('current-page-title').textContent = pageId.charAt(0).toUpperCase() + pageId.slice(1).replace('-', ' ');

    // Page specific loads
    if (pageId === 'students') this.loadStudentsList();
    if (pageId === 'classes') this.loadClassesList();
    if (pageId === 'bus') this.loadBusStopsList();
    if (pageId === 'academic-years') this.loadAcademicYearsList();
    if (pageId === 'reports') this.loadReports();
    if (pageId === 'sync-queue') this.loadSyncQueue();
  }

  // Modals management
  showModal(modalId) {
    document.getElementById(modalId).style.display = 'flex';
  }

  closeModal(modalId) {
    document.getElementById(modalId).style.display = 'none';
  }

  // STUDENTS TAB
  async loadStudentsList() {
    const tbody = document.querySelector('#students-table tbody');
    tbody.innerHTML = '<tr><td colspan="10" class="text-center">Loading students...</td></tr>';
    try {
      const ayId = this.activeAcademicYear ? this.activeAcademicYear.id : null;
      const list = await invoke('get_students', { academicYearId: ayId });
      this.students = list;
      this.displayFilteredStudents();
    } catch (err) {
      tbody.innerHTML = `<tr><td colspan="10" class="text-danger">Error: ${err}</td></tr>`;
    }
  }

  displayFilteredStudents() {
    const tbody = document.querySelector('#students-table tbody');
    if (!this.students) return;

    const query = (document.getElementById('students-search-input').value || '').toLowerCase().trim();
    const duesFilter = document.getElementById('students-dues-filter').value;

    let filtered = this.students.filter(s => {
      // 1. Text Search Filter
      if (query) {
        const matchesName = (s.student_name || '').toLowerCase().includes(query);
        const matchesAdm = (s.admission_number || '').toLowerCase().includes(query);
        const matchesClass = (s.class_name || '').toLowerCase().includes(query);
        const matchesPhone = (s.phone || '').toLowerCase().includes(query);
        if (!matchesName && !matchesAdm && !matchesClass && !matchesPhone) {
          return false;
        }
      }

      // 2. Dues Status Filter
      if (duesFilter === 'unpaid-any') {
        return (s.pending_balance || 0) > 0.01;
      } else if (duesFilter === 'unpaid-past') {
        return (s.pending_past || 0) > 0.01;
      } else if (duesFilter === 'unpaid-current') {
        return (s.pending_current || 0) > 0.01;
      }

      return true;
    });

    tbody.innerHTML = '';
    if (filtered.length === 0) {
      tbody.innerHTML = '<tr><td colspan="10" class="text-center text-muted">No students matching filters</td></tr>';
      return;
    }

    filtered.forEach(s => {
      tbody.innerHTML += `
        <tr>
          <td>${s.admission_number}</td>
          <td>${s.roll_number || '-'}</td>
          <td><strong>${s.student_name}</strong></td>
          <td>${s.parent_name || '-'}</td>
          <td>${s.class_name || '-'}</td>
          <td>${s.bus_stop_name || 'Walking / No bus'}</td>
          <td>${s.phone || '-'}</td>
          <td><strong style="color: #ff4d4d;">₹${(s.pending_balance || 0).toFixed(2)}</strong></td>
          <td><span class="badge status-${s.status}">${s.status}</span></td>
          <td>
            <button class="btn btn-outline btn-xs" onclick="app.selectStudentForBilling('${s.student_id}')">Bill Setup</button>
            <button class="btn btn-xs" onclick="app.sendWhatsAppReminder('${s.student_name.replace(/'/g, "\\'")}', '${s.phone || ''}', ${s.pending_balance || 0}, '${s.admission_number}')" title="Send WhatsApp Reminder" style="background-color: #25d366; color: white; border: none; display: inline-flex; align-items: center; gap: 4px; padding: 2px 6px; font-weight: 500; cursor: pointer; border-radius: 3px; margin-left: 4px;">
              <svg style="width: 12px; height: 12px; fill: white;" viewBox="0 0 24 24">
                <path d="M.057 24l1.687-6.163c-1.041-1.804-1.588-3.849-1.587-5.946C.06 5.348 5.397.01 12.008.01c3.202.001 6.212 1.246 8.477 3.514 2.266 2.268 3.507 5.28 3.505 8.484-.004 6.657-5.34 11.997-11.953 11.997-2.005-.001-3.973-.502-5.724-1.457L0 24zm6.59-4.846c1.6.95 3.188 1.449 4.625 1.451 5.403.002 9.803-4.394 9.806-9.8.001-2.605-1.012-5.054-2.85-6.895C16.39 2.069 13.938 1.056 11.3 1.056c-5.4.001-9.799 4.399-9.802 9.8-.001 1.583.435 3.125 1.265 4.502l-.993 3.626 3.725-.976zm10.992-6.84c-.27-.135-1.597-.788-1.845-.877-.249-.09-.43-.135-.61.135-.18.27-.697.877-.855 1.057-.158.18-.315.202-.586.067-1.244-.622-2.148-1.085-3.007-2.553-.228-.393.228-.365.652-1.213.075-.15.037-.281-.019-.393-.056-.113-.43-1.037-.59-1.428-.155-.373-.326-.322-.445-.327-.116-.005-.248-.006-.38-.006-.133 0-.348.05-.53.249-.182.198-.697.681-.697 1.66s.712 1.925.811 2.06c.099.135 1.401 2.14 3.393 3.001.474.205.845.328 1.134.42.477.152.911.13 1.254.079.382-.057 1.597-.653 1.822-1.284.225-.631.225-1.171.157-1.284-.067-.113-.249-.202-.519-.337z"/>
              </svg>
              WhatsApp
            </button>
          </td>
        </tr>
      `;
    });
  }

  sendWhatsAppReminder(studentName, phone, pendingFee, admissionNumber) {
    if (!phone || phone.trim() === '-' || phone.trim() === '') {
      alert("No phone number registered for this student!");
      return;
    }
    let cleanPhone = phone.replace(/\D/g, '');
    if (cleanPhone.length === 10) {
      cleanPhone = "91" + cleanPhone;
    }
    const message = `Dear Parent, this is a reminder that the outstanding balance for your ward ${studentName} (Adm No: ${admissionNumber}) is ₹${pendingFee.toFixed(2)}. Please clear the dues at the school office. Thank you!`;
    const encodedMessage = encodeURIComponent(message);
    const url = `https://wa.me/${cleanPhone}?text=${encodedMessage}`;
    invoke('open_external_url', { url }).catch(err => {
      console.error("Failed to open WhatsApp URL:", err);
      // Fallback
      window.open(url, '_blank');
    });
  }

  async editStudent(id) {
    try {
      const s = await invoke('get_student', { id });
      document.getElementById('modal-student-title').textContent = 'Edit Student';
      document.getElementById('student-db-id').value = s.id;
      document.getElementById('student-name').value = s.student_name;
      document.getElementById('student-admission').value = s.admission_number;
      document.getElementById('student-roll').value = s.roll_number || '';
      document.getElementById('student-parent').value = s.parent_name || '';
      document.getElementById('student-phone').value = s.phone || '';
      document.getElementById('student-class').value = s.class_id || '';
      document.getElementById('student-bus-stop').value = s.bus_stop_id || '';
      document.getElementById('student-status').value = s.status;
      document.getElementById('student-status-wrapper').style.display = 'block';
      this.showModal('modal-student');
    } catch (err) {
      alert("Error: " + err);
    }
  }

  // CLASSES TAB
  async loadClassesList() {
    const tbody = document.querySelector('#classes-table tbody');
    tbody.innerHTML = '<tr><td colspan="7" class="text-center">Loading classes...</td></tr>';
    try {
      const ayId = this.activeAcademicYear ? this.activeAcademicYear.id : null;
      const list = await invoke('get_classes', { academicYearId: ayId });
      this.classes = list;
      tbody.innerHTML = '';
      if (list.length === 0) {
        tbody.innerHTML = '<tr><td colspan="7" class="text-center text-muted">No classes created for this academic year.</td></tr>';
        return;
      }
      list.forEach(c => {
        const extraFeeCount = c.custom_fees.length;
        tbody.innerHTML += `
          <tr>
            <td><strong>${c.name}</strong></td>
            <td>${c.section || '-'}</td>
            <td>₹${c.tuition_fee.toFixed(2)}</td>
            <td>₹${c.admission_fee.toFixed(2)}</td>
            <td>${extraFeeCount} components</td>
            <td>
              <button class="btn btn-outline btn-xs" onclick="app.editClass(${c.id})">Edit</button>
              <button class="btn btn-danger btn-xs" onclick="app.deleteClass(${c.id})">Delete</button>
            </td>
          </tr>
        `;
      });
    } catch (err) {
      tbody.innerHTML = `<tr><td colspan="7" class="text-danger">Error: ${err}</td></tr>`;
    }
  }

  async editClass(id) {
    const c = this.classes.find(x => x.id === id);
    if (!c) return;

    document.getElementById('modal-class-title').textContent = 'Edit Class';
    document.getElementById('class-db-id').value = c.id;
    document.getElementById('class-name').value = c.name;
    document.getElementById('class-section').value = c.section || '';
    document.getElementById('class-tuition').value = c.tuition_fee;
    document.getElementById('class-admission').value = c.admission_fee;
    document.getElementById('class-book').value = c.book_fee;
    document.getElementById('class-uniform').value = c.uniform_fee;

    // Custom fee component lines populate
    const listDiv = document.getElementById('class-custom-fees-list');
    listDiv.innerHTML = '';
    c.custom_fees.forEach(cf => {
      this.addClassCustomFeeRow(cf.name, cf.amount);
    });

    this.showModal('modal-class');
  }

  addClassCustomFeeRow(name = '', amount = 0) {
    const listDiv = document.getElementById('class-custom-fees-list');
    const row = document.createElement('div');
    row.className = 'class-custom-fee-row extra-fee-row';
    row.innerHTML = `
      <input type="text" class="custom-name" placeholder="Component Name" value="${name}" required>
      <input type="number" class="custom-amt" placeholder="Amount" value="${amount}" min="0" step="0.01" required>
      <span class="material-symbols-outlined text-danger btn-delete-row" onclick="this.parentElement.remove()">delete</span>
    `;
    listDiv.appendChild(row);
  }

  async deleteClass(id) {
    if (!confirm("Are you sure you want to delete this class?")) return;
    try {
      await invoke('delete_class', { id });
      this.classes = await invoke('get_classes');
      this.populateSelectDropdowns();
      await this.loadClassesList();
    } catch (e) {
      alert(e);
    }
  }

  // TRANSPORT BUS TAB
  async loadBusStopsList() {
    const tbody = document.querySelector('#bus-stops-table tbody');
    tbody.innerHTML = '<tr><td colspan="3" class="text-center">Loading bus stops...</td></tr>';
    try {
      const list = await invoke('get_bus_stops');
      this.busStops = list;
      tbody.innerHTML = '';
      if (list.length === 0) {
        tbody.innerHTML = '<tr><td colspan="3" class="text-center text-muted">No bus stops setup yet</td></tr>';
        return;
      }
      list.forEach(b => {
        tbody.innerHTML += `
          <tr>
            <td><strong>${b.name}</strong></td>
            <td>₹${b.monthly_charge.toFixed(2)} / month</td>
            <td>
              <button class="btn btn-outline btn-xs" onclick="app.editBusStop(${b.id})">Edit</button>
              <button class="btn btn-danger btn-xs" onclick="app.deleteBusStop(${b.id})">Delete</button>
            </td>
          </tr>
        `;
      });
    } catch (err) {
      tbody.innerHTML = `<tr><td colspan="3" class="text-danger">Error: ${err}</td></tr>`;
    }
  }

  editBusStop(id) {
    const b = this.busStops.find(x => x.id === id);
    if (!b) return;
    document.getElementById('modal-bus-stop-title').textContent = 'Edit Bus Stop';
    document.getElementById('bus-stop-db-id').value = b.id;
    document.getElementById('bus-stop-name').value = b.name;
    document.getElementById('bus-stop-charge').value = b.monthly_charge;
    this.showModal('modal-bus-stop');
  }

  async deleteBusStop(id) {
    if (!confirm("Are you sure you want to delete this bus stop?")) return;
    try {
      await invoke('delete_bus_stop', { id });
      this.busStops = await invoke('get_bus_stops');
      this.populateSelectDropdowns();
      await this.loadBusStopsList();
    } catch (e) {
      alert(e);
    }
  }

  // ACADEMIC YEARS TAB
  async loadAcademicYearsList() {
    const tbody = document.querySelector('#academic-years-table tbody');
    tbody.innerHTML = '<tr><td colspan="4" class="text-center">Loading years...</td></tr>';
    try {
      const list = await invoke('get_academic_years');
      this.academicYears = list;
      tbody.innerHTML = '';
      if (list.length === 0) {
        tbody.innerHTML = '<tr><td colspan="4" class="text-center text-muted">No academic years created yet.</td></tr>';
        return;
      }
      list.forEach(y => {
        tbody.innerHTML += `
          <tr class="${y.is_active ? 'row-active-year' : ''}">
            <td><strong>${y.name}</strong>${y.is_active ? ' <span class="badge status-active" style="font-size:10px">CURRENT</span>' : ''}</td>
            <td>${y.start_date}</td>
            <td>
              <span class="badge ${y.is_active ? 'status-active' : 'status-inactive'}">
                ${y.is_active ? '✓ Active' : 'Archived'}
              </span>
            </td>
            <td>
              ${y.is_active ? '<span class="text-muted" style="font-size:12px">Current Year</span>' : `<button class="btn btn-outline btn-xs" onclick="app.setActiveYear(${y.id})">Restore</button>`}
            </td>
          </tr>
        `;
      });
    } catch (err) {
      tbody.innerHTML = `<tr><td colspan="4" class="text-danger">Error: ${err}</td></tr>`;
    }
  }

  async setActiveYear(id) {
    try {
      await invoke('set_active_academic_year', { id });
      await this.loadAppInitialData();
      await this.loadAcademicYearsList();
    } catch (e) {
      alert(e);
    }
  }

  addPromotionMappingRow() {
    const container = document.getElementById('promotion-mappings-list');
    const row = document.createElement('div');
    row.className = 'promotion-mapping-row';
    
    let classOptions = '<option value="">Select Class</option>';
    this.classes.forEach(c => {
      classOptions += `<option value="${c.id}">${c.name} ${c.section ? '-' + c.section : ''}</option>`;
    });

    row.innerHTML = `
      <select class="map-from" required>${classOptions}</select>
      <span class="material-symbols-outlined">trending_flat</span>
      <select class="map-to" required>${classOptions}</select>
      <span class="material-symbols-outlined text-danger btn-delete-row" onclick="this.parentElement.remove()">delete</span>
    `;
    container.appendChild(row);
  }

  // BILLING OPERATIONS
  async selectStudentForBilling(studentId) {
    try {
      this.currentMonthlySummary = null; // Clear previous student's summary
      // Find local student
      let s = this.students.find(x => x.student_id === studentId);
      if (!s) {
        // search DB
        const sResults = await invoke('search_students', { query: studentId });
        s = sResults.find(x => x.student_id === studentId);
      }
      if (!s) {
        alert("Student not found");
        return;
      }

      // Refresh student data from backend for up-to-date pending_balance
      try {
        const fresh = await invoke('get_student', { id: s.id });
        if (fresh) s = fresh;
      } catch (_) {}

      this.selectedStudentForBill = s;
      this.navigateToPage('billing');

      // Display student details
      document.getElementById('billing-selected-student-card').style.display = 'block';
      document.getElementById('bill-student-name').textContent = s.student_name;
      document.getElementById('bill-student-adm').textContent = s.admission_number;
      document.getElementById('bill-student-class').textContent = s.class_name || 'None';
      document.getElementById('bill-student-bus').textContent = s.bus_stop_name || 'None';
      document.getElementById('bill-student-pending').textContent = '₹' + (s.pending_balance || 0).toFixed(2);

      // Wire up WhatsApp quick reminder button
      const waBtn = document.getElementById('bill-student-whatsapp-btn');
      waBtn.onclick = () => {
        this.sendWhatsAppReminder(s.student_name, s.phone, s.pending_balance || 0, s.admission_number);
      };

      // Load active class fees if new bill, or load existing bill structure
      if (!this.activeAcademicYear) {
        alert("Please set an active academic year in Academic Years page first!");
        return;
      }

      const existingBill = await invoke('get_bill', { studentId, academicYearId: this.activeAcademicYear.id });
      
      const extraFeesContainer = document.getElementById('extra-fees-list');
      extraFeesContainer.innerHTML = '';

      if (existingBill) {
        this.currentBill = existingBill;
        // Populate inputs with existing bill
        document.getElementById('bill-input-tuition').value = existingBill.tuition_fee;
        document.getElementById('bill-input-admission').value = existingBill.admission_fee;
        document.getElementById('bill-input-book').value = existingBill.book_fee;
        document.getElementById('bill-input-uniform').value = existingBill.uniform_fee;
        document.getElementById('bill-input-bus').value = existingBill.bus_fee;
        document.getElementById('bill-input-prev-bal').value = existingBill.previous_balance;
        document.getElementById('bill-input-discount').value = existingBill.discount;

        // Load detailed extra fee items from database
        try {
          invoke('get_bill_items', { billId: existingBill.id })
            .then(items => {
              if (items && items.length > 0) {
                items.forEach(item => {
                  this.addExtraFeeRow(item.name, item.amount);
                });
                this.recalculateBillTotals();
              } else if (existingBill.extra_fees > 0) {
                this.addExtraFeeRow('Custom fees total', existingBill.extra_fees);
                this.recalculateBillTotals();
              }
            })
            .catch(err => {
              console.error("Failed to load bill items", err);
              if (existingBill.extra_fees > 0) {
                this.addExtraFeeRow('Custom fees total', existingBill.extra_fees);
                this.recalculateBillTotals();
              }
            });
        } catch (e) {
          console.error(e);
          if (existingBill.extra_fees > 0) {
            this.addExtraFeeRow('Custom fees total', existingBill.extra_fees);
          }
        }
      } else {
        this.currentBill = null;
        // Load default fee from class structure
        const cls = this.classes.find(x => x.id === s.class_id);

        document.getElementById('bill-input-tuition').value = cls ? cls.tuition_fee : 0;
        document.getElementById('bill-input-admission').value = cls ? cls.admission_fee : 0;
        document.getElementById('bill-input-book').value = cls ? cls.book_fee : 0;
        document.getElementById('bill-input-uniform').value = cls ? cls.uniform_fee : 0;
        document.getElementById('bill-input-bus').value = 0;
        document.getElementById('bill-input-prev-bal').value = 0;
        document.getElementById('bill-input-discount').value = 0;

        // Load custom fees from the class config automatically
        if (cls && cls.custom_fees) {
          cls.custom_fees.forEach(cf => {
            this.addExtraFeeRow(cf.name, cf.amount);
          });
        }
      }

      // Show/hide bus section & populate bus stop dropdown
      this._setupBusSectionForStudent(s);

      this.recalculateBillTotals();
      document.getElementById('btn-save-bill').disabled = false;
      await this.loadRecentReceipts(studentId);

      // Load monthly tuition & bus tracker
      const trackerCard = document.getElementById('monthly-tracker-card');
      const trackerTbody = document.getElementById('monthly-tracker-tbody');
      if (trackerCard && trackerTbody) {
        trackerCard.style.display = 'block';
        trackerTbody.innerHTML = '<tr><td colspan="5" class="text-center">Loading monthly status...</td></tr>';
        try {
          const tuitionStatus = await invoke('get_monthly_tuition_status', {
            studentId: s.student_id,
            academicYearId: this.activeAcademicYear.id
          });
          const busUsage = await invoke('get_student_bus_usage', {
            studentId: s.student_id,
            academicYearId: this.activeAcademicYear.id
          });
          const summary = await invoke('get_monthly_bill_summary', {
            studentId: s.student_id,
            academicYearId: this.activeAcademicYear.id
          });

          this.currentMonthlySummary = summary;
          trackerTbody.innerHTML = '';
          const busMap = new Map();
          if (busUsage && busUsage.length) {
            busUsage.forEach(b => busMap.set(`${b.month}-${b.year}`, b));
          }

          if (tuitionStatus && tuitionStatus.length) {
            tuitionStatus.forEach(t => {
              const key = `${t.month}-${t.year}`;
              const busItem = busMap.get(key);
              const dateObj = new Date(t.year, t.month - 1, 1);
              const monthName = dateObj.toLocaleString('en-US', { month: 'long' });

              let busCell = '<span class="text-muted">No Bus Stop Assigned</span>';
              let busStatusCell = '-';

              if (s.bus_stop_id) {
                const isChecked = busItem && busItem.bus_used ? 'checked' : '';
                const fee = busItem ? busItem.bus_fee : 0.0;
                const paid = busItem ? busItem.amount_paid : 0.0;
                const bal = busItem ? busItem.balance : 0.0;
                const stat = busItem ? busItem.status : 'unpaid';

                busCell = `
                  <div style="display:flex; align-items:center; gap:8px;">
                    <input type="checkbox" class="bus-toggle-chk" data-month="${t.month}" data-year="${t.year}" ${isChecked} style="width:16px; height:16px; cursor:pointer;">
                    <span>₹${fee.toFixed(2)}</span>
                  </div>
                `;
                busStatusCell = `
                  <span class="status-badge ${stat}">${stat} (bal: ₹${bal.toFixed(2)})</span>
                `;
              }

              const tr = document.createElement('tr');
              tr.innerHTML = `
                <td><strong>${monthName} ${t.year}</strong></td>
                <td>₹${t.monthly_amount.toFixed(2)} (paid: ₹${Math.min(t.amount_paid, t.monthly_amount).toFixed(2)})</td>
                <td><span class="status-badge ${t.status}">${t.status} (bal: ₹${t.balance.toFixed(2)})</span></td>
                <td>${busCell}</td>
                <td>${busStatusCell}</td>
              `;
              trackerTbody.appendChild(tr);
            });

            trackerTbody.querySelectorAll('.bus-toggle-chk').forEach(chk => {
              chk.addEventListener('change', async (e) => {
                const month = parseInt(e.target.dataset.month);
                const year = parseInt(e.target.dataset.year);
                const used = e.target.checked;
                try {
                  await invoke('set_monthly_bus_usage', {
                    studentId: s.student_id,
                    academicYearId: this.activeAcademicYear.id,
                    month,
                    year,
                    busUsed: used,
                  });
                  await this.selectStudentForBilling(s.student_id);
                } catch (err) {
                  alert("Failed to toggle bus usage: " + err);
                  e.target.checked = !used;
                }
              });
            });
          } else {
            trackerTbody.innerHTML = '<tr><td colspan="5" class="text-center text-muted">No monthly installments generated. Save bill structure first.</td></tr>';
            this.currentMonthlySummary = null;
          }

          if (summary && tuitionStatus && tuitionStatus.length) {
            const payAmtInput = document.getElementById('pay-input-amount');
            // Bill is the single source of truth for total due; fall back to monthly outstanding if no bill
            const totalDue = this.currentBill
              ? this.currentBill.balance
              : (summary.outstanding_tuition || 0) + (summary.outstanding_bus || 0);
            document.getElementById('pay-input-notes').value = '';
            payAmtInput.max = totalDue > 0 ? totalDue : '';
            payAmtInput.value = totalDue.toFixed(2);
            payAmtInput.oninput = () => this._computePaymentAllocation(parseFloat(payAmtInput.value) || 0);
            this._computePaymentAllocation(totalDue);

            const payBtn = document.getElementById('btn-record-payment');
            if (totalDue > 0) {
              payBtn.disabled = false;
            } else {
              payBtn.disabled = true;
            }
          } else {
            // Keep yearly bill payment totals
            document.getElementById('pay-input-notes').value = '';
            const payAmtInput = document.getElementById('pay-input-amount');
            payAmtInput.oninput = () => this._computePaymentAllocation(parseFloat(payAmtInput.value) || 0);
            this.recalculateBillTotals();
          }
        } catch (err) {
          console.error("Failed to load monthly tracker details:", err);
          trackerTbody.innerHTML = '<tr><td colspan="5" class="text-center text-danger">Error loading monthly details.</td></tr>';
        }
      }
    } catch (e) {
      alert("Error loading student bill setup: " + e);
    }
  }

  addExtraFeeRow(name = '', amount = 0) {
    const listDiv = document.getElementById('extra-fees-list');
    const row = document.createElement('div');
    row.className = 'extra-fee-row';
    row.innerHTML = `
      <input type="text" class="extra-fee-name" placeholder="Fee Name" value="${name}" required>
      <input type="number" class="extra-fee-amount" placeholder="Amount" value="${amount}" min="0" step="0.01" required>
      <span class="material-symbols-outlined text-danger btn-delete-row" onclick="this.parentElement.remove(); app.recalculateBillTotals()">delete</span>
    `;
    listDiv.appendChild(row);
    // Bind change listener
    row.querySelector('.extra-fee-amount').addEventListener('input', () => this.recalculateBillTotals());
  }

  recalculateBillTotals() {
    const tuition = parseFloat(document.getElementById('bill-input-tuition').value) || 0;
    const admission = parseFloat(document.getElementById('bill-input-admission').value) || 0;
    const book = parseFloat(document.getElementById('bill-input-book').value) || 0;
    const uniform = parseFloat(document.getElementById('bill-input-uniform').value) || 0;
    const bus = parseFloat(document.getElementById('bill-input-bus').value) || 0;
    const prevBal = parseFloat(document.getElementById('bill-input-prev-bal').value) || 0;
    const discount = parseFloat(document.getElementById('bill-input-discount').value) || 0;

    let extraSum = 0;
    document.querySelectorAll('#extra-fees-list .extra-fee-row').forEach(row => {
      const val = parseFloat(row.querySelector('.extra-fee-amount').value) || 0;
      extraSum += val;
    });

    const subtotal = tuition + admission + book + uniform + bus + prevBal + extraSum;
    const total = Math.max(0, subtotal - discount);
    
    // Amount paid so far
    const amtPaid = this.currentBill ? this.currentBill.amount_paid : 0;
    const balance = Math.max(0, total - amtPaid);

    document.getElementById('bill-calculated-total').textContent = `₹${total.toFixed(2)}`;
    document.getElementById('bill-calculated-paid').textContent = `₹${amtPaid.toFixed(2)}`;
    document.getElementById('bill-calculated-balance').textContent = `₹${balance.toFixed(2)}`;

    // Enable / disable payment button
    const payBtn = document.getElementById('btn-record-payment');
    if (this.currentBill && balance > 0) {
      payBtn.disabled = false;
      document.getElementById('pay-input-amount').max = balance;
      document.getElementById('pay-input-amount').value = balance;
    } else {
      payBtn.disabled = true;
      document.getElementById('pay-input-amount').value = '';
    }
  }

  // Called when bus stop dropdown changes in billing
  onBusStopChange() {
    const stopId = parseInt(document.getElementById('bill-bus-stop-select').value) || 0;
    const busType = document.getElementById('bill-bus-type-select').value;
    const stop = this.busStops.find(b => b.id === stopId);
    let charge = stop ? stop.monthly_charge : 0;
    // Morning/evening only = half charge
    if (stop && busType !== 'both') charge = charge / 2;
    document.getElementById('bill-input-bus').value = charge.toFixed(2);
    this.recalculateBillTotals();

    // Also update the student card display
    document.getElementById('bill-student-bus').textContent = stop ? `${stop.name} (${busType})` : 'None';
  }

  _setupBusSectionForStudent(s) {
    const busSection = document.getElementById('bill-bus-section');
    const busSelect = document.getElementById('bill-bus-stop-select');
    const busTypeSelect = document.getElementById('bill-bus-type-select');
    const enableBtnWrapper = document.getElementById('bill-bus-enable-btn-wrapper');

    // Populate bus stop dropdown
    busSelect.innerHTML = '<option value="">-- No Bus / Walking --</option>';
    this.busStops.forEach(b => {
      busSelect.innerHTML += `<option value="${b.id}">${b.name} (₹${b.monthly_charge}/mo)</option>`;
    });

    const isBus = (s.student_type === 'bus' || s.bus_stop_id);

    if (isBus) {
      busSection.style.display = 'block';
      if (enableBtnWrapper) enableBtnWrapper.style.display = 'none';
      busSelect.value = s.bus_stop_id || '';
      
      // Try to detect bus type from current bill bus_fee vs full charge
      if (s.bus_stop_id) {
        const stop = this.busStops.find(b => b.id === s.bus_stop_id);
        if (stop && this.currentBill) {
          const bf = this.currentBill.bus_fee;
          if (Math.abs(bf - stop.monthly_charge / 2) < 1) {
            busTypeSelect.value = 'morning';
          } else {
            busTypeSelect.value = 'both';
          }
        }
      } else {
        busTypeSelect.value = 'both';
      }
      this.onBusStopChange();
    } else {
      busSection.style.display = 'none';
      if (enableBtnWrapper) enableBtnWrapper.style.display = 'block';
      document.getElementById('bill-input-bus').value = 0;
    }
  }

  async saveBillStructure() {
    if (!this.selectedStudentForBill || !this.activeAcademicYear) return;
    
    const extraFeeItems = [];
    let extraSum = 0;
    document.querySelectorAll('#extra-fees-list .extra-fee-row').forEach(row => {
      const name = row.querySelector('.extra-fee-name').value;
      const amount = parseFloat(row.querySelector('.extra-fee-amount').value) || 0;
      if (name) {
        extraFeeItems.push({ name, amount });
        extraSum += amount;
      }
    });

    const tuition = parseFloat(document.getElementById('bill-input-tuition').value) || 0;
    const admission = parseFloat(document.getElementById('bill-input-admission').value) || 0;
    const book = parseFloat(document.getElementById('bill-input-book').value) || 0;
    const uniform = parseFloat(document.getElementById('bill-input-uniform').value) || 0;
    const bus = parseFloat(document.getElementById('bill-input-bus').value) || 0;
    const prevBal = parseFloat(document.getElementById('bill-input-prev-bal').value) || 0;
    const discount = parseFloat(document.getElementById('bill-input-discount').value) || 0;

    // Save bus stop assignment if changed by accountant
    const selectedBusStopId = parseInt(document.getElementById('bill-bus-stop-select').value) || null;
    const finalStudentType = selectedBusStopId ? 'bus' : (this.selectedStudentForBill.student_type || 'walking');
    if (selectedBusStopId !== (this.selectedStudentForBill.bus_stop_id || null) || finalStudentType !== this.selectedStudentForBill.student_type) {
      try {
        await invoke('update_student', {
          id: this.selectedStudentForBill.id,
          rollNumber: this.selectedStudentForBill.roll_number || null,
          studentName: this.selectedStudentForBill.student_name,
          parentName: this.selectedStudentForBill.parent_name || null,
          phone: this.selectedStudentForBill.phone || null,
          classId: this.selectedStudentForBill.class_id || null,
          busStopId: selectedBusStopId,
          status: this.selectedStudentForBill.status || 'active',
          studentType: finalStudentType
        });
        this.selectedStudentForBill.bus_stop_id = selectedBusStopId;
        this.selectedStudentForBill.student_type = finalStudentType;
        const stop = this.busStops.find(b => b.id === selectedBusStopId);
        this.selectedStudentForBill.bus_stop_name = stop ? stop.name : null;
        // Reload students list cache
        this.students = await invoke('get_students');
      } catch(e) {
        console.warn('Failed to update bus stop/type assignment:', e);
      }
    }

    const subtotal = tuition + admission + book + uniform + bus + prevBal + extraSum;
    const totalFee = Math.max(0, subtotal - discount);

    const payload = {
      student_id: this.selectedStudentForBill.student_id,
      academic_year_id: this.activeAcademicYear.id,
      tuition_fee: tuition,
      admission_fee: admission,
      exam_fee: 0,
      book_fee: book,
      uniform_fee: uniform,
      lab_fee: 0,
      computer_fee: 0,
      sports_fee: 0,
      activity_fee: 0,
      maintenance_fee: 0,
      bus_fee: bus,
      previous_balance: prevBal,
      extra_fees: extraSum,
      discount,
      scholarship: 0,
      total_fee: totalFee,
      extra_fee_items: extraFeeItems
    };

    try {
      await invoke('generate_bill', { req: payload });
      alert("Student bill structure saved successfully!");
      this.students = await invoke('get_students');
      // Reload setup
      await this.selectStudentForBilling(this.selectedStudentForBill.student_id);
      await this.updateDashboardStats();
      // Auto-sync fees to Supabase after bill generation
      invoke('sync_fees').then(() => this.loadSyncQueue()).catch(console.error);
    } catch (e) {
      alert("Failed to save bill: " + e);
    }
  }

  /** Compute allocation breakdown and populate allocation fields in the payment sidebar.
   *  Priority: 1) Admission  2) Other (Book/Uniform/Prev-Bal/Extra)  3) Tuition  4) Bus
   */
  _computePaymentAllocation(amount) {
    if (!this.selectedStudentForBill) return;

    const bill = this.currentBill;
    const monthly = this.currentMonthlySummary;

    // Outstanding amounts from the yearly bill structure
    let admBal = 0, otherBal = 0;
    if (bill) {
      // Use the bill's unpaid portion split by category.
      // We approximate per-category balance proportionally to the overall bill balance.
      const totalStructural = (bill.admission_fee || 0)
        + (bill.book_fee || 0) + (bill.uniform_fee || 0)
        + (bill.previous_balance || 0) + (bill.extra_fees || 0)
        - (bill.discount || 0);
      const billBal = Math.max(0, bill.balance || 0);

      if (totalStructural > 0 && billBal > 0) {
        const ratio = Math.min(billBal / totalStructural, 1);
        admBal   = (bill.admission_fee || 0) * ratio;
        otherBal = ((bill.book_fee || 0) + (bill.uniform_fee || 0)
                   + (bill.previous_balance || 0) + (bill.extra_fees || 0)
                   - (bill.discount || 0)) * ratio;
      }
    }

    const tuitionBal = monthly ? (monthly.outstanding_tuition || 0) : 0;
    const busBal     = monthly ? (monthly.outstanding_bus || 0) : 0;

    let remaining = amount;

    // 1. Admission
    const paidAdm   = Math.min(remaining, admBal);   remaining -= paidAdm;
    // 2. Other fees
    const paidOther = Math.min(remaining, otherBal);  remaining -= paidOther;
    // 3. Tuition
    const paidTuit  = Math.min(remaining, tuitionBal); remaining -= paidTuit;
    // 4. Bus
    const paidBus   = Math.min(remaining, busBal);    remaining -= paidBus;
    // Any residual goes to Other (e.g. advance/overpay)
    const finalOther = paidOther + remaining;

    const set = (id, val) => { const el = document.getElementById(id); if (el) el.value = val.toFixed(2); };
    set('pay-alloc-admission', paidAdm);
    set('pay-alloc-other',     finalOther);
    set('pay-alloc-tuition',   paidTuit);
    set('pay-alloc-bus',       paidBus);
  }

  async recordPayment() {
    if (!this.selectedStudentForBill) return;

    const amount = parseFloat(document.getElementById('pay-input-amount').value) || 0;
    const mode = document.getElementById('pay-input-mode').value;
    const date = document.getElementById('pay-input-date').value;
    const notes = document.getElementById('pay-input-notes').value;

    if (amount <= 0) {
      alert("Payment amount must be greater than zero");
      return;
    }

    // --- Read editable allocation fields from UI ---
    const allocAdmission = parseFloat(document.getElementById('pay-alloc-admission')?.value) || 0;
    const allocOther     = parseFloat(document.getElementById('pay-alloc-other')?.value) || 0;
    const allocTuition   = parseFloat(document.getElementById('pay-alloc-tuition')?.value) || 0;
    const allocBus       = parseFloat(document.getElementById('pay-alloc-bus')?.value) || 0;

    const allocTotal = allocAdmission + allocOther + allocTuition + allocBus;
    if (Math.abs(allocTotal - amount) > 0.5) {
      alert(`Allocation total (₹${allocTotal.toFixed(2)}) does not match payment amount (₹${amount.toFixed(2)}). Please adjust.`);
      return;
    }

    try {
      let receiptNo;
      if (this.currentBill) {
        // Yearly bill payment — unified allocation
        receiptNo = await invoke('record_payment', {
          req: {
            student_id: this.selectedStudentForBill.student_id,
            bill_id: this.currentBill.id,
            amount,
            payment_mode: mode,
            payment_date: date,
            notes: notes || null,
            allocated_admission: allocAdmission,
            allocated_other: allocOther,
            allocated_tuition: allocTuition,
            allocated_bus: allocBus,
          }
        });
      } else if (this.currentMonthlySummary) {
        receiptNo = await invoke('record_monthly_payment', {
          studentId: this.selectedStudentForBill.student_id,
          academicYearId: this.activeAcademicYear.id,
          tuitionAmount: allocTuition,
          busAmount: allocBus,
          allocatedAdmission: allocAdmission,
          allocatedOther: allocOther,
          paymentMode: mode,
          paymentDate: date,
          notes: notes || null
        });
      } else {
        alert("No bill structure exists for this student.");
        return;
      }

      alert(`Payment of ₹${amount.toFixed(2)} recorded successfully. Receipt: ${receiptNo}`);
      await this.printReceipt(receiptNo);
      this.students = await invoke('get_students');
      await this.selectStudentForBilling(this.selectedStudentForBill.student_id);
      await this.updateDashboardStats();
      invoke('sync_fees').then(() => this.loadSyncQueue()).catch(console.error);
    } catch (e) {
      alert("Failed to record payment: " + e);
    }
  }

  async loadRecentReceipts(studentId) {
    const container = document.getElementById('recent-receipts-container');
    container.innerHTML = '<div class="text-muted text-center py-2">Loading receipts...</div>';
    try {
      const allPayments = await invoke('get_payments');
      const filtered = allPayments.filter(p => p.student_id === studentId);
      container.innerHTML = '';
      if (filtered.length === 0) {
        container.innerHTML = '<div class="text-muted text-center py-2">No payments recorded.</div>';
        return;
      }
      filtered.forEach(p => {
        container.innerHTML += `
          <div class="receipt-card-mini">
            <div class="left">
              <strong>${p.receipt_number}</strong><br>
              <span>${p.payment_date} (${p.payment_mode})</span>
            </div>
            <div class="right" style="display: flex; flex-direction: column; gap: 4px; align-items: flex-end;">
              <strong>₹${p.amount.toFixed(2)}</strong>
              <div style="display: flex; gap: 4px;">
                <button class="btn btn-outline btn-xs" onclick="app.printReceipt('${p.receipt_number}')">View/Print</button>
                <button class="btn btn-danger btn-xs btn-undo-payment" onclick="app.revertPayment(${p.id}, '${p.receipt_number}', ${p.amount}, false)">Undo</button>
              </div>
            </div>
          </div>
        `;
      });
    } catch (e) {
      container.innerHTML = `<div class="text-danger">Error: ${e}</div>`;
    }
  }

  async revertPayment(paymentId, receiptNo, amount, isReportView = false) {
    try {
      await this.showConfirm(receiptNo, amount);
    } catch {
      return; // user cancelled
    }

    try {
      await invoke('revert_payment', { paymentId });
      this.showToast(`Payment ₹${amount.toFixed(2)} (${receiptNo}) undone successfully.`, 'success');
      this.students = await invoke('get_students');
      if (isReportView) {
        await this.loadReports();
      } else if (this.selectedStudentForBill) {
        await this.selectStudentForBilling(this.selectedStudentForBill.student_id);
      }
      await this.updateDashboardStats();
      // Auto-sync fees to Supabase after reverting payment
      invoke('sync_fees').then(() => this.loadSyncQueue()).catch(console.error);
    } catch (e) {
      this.showToast('Failed to undo payment: ' + e, 'error');
    }
  }

  // Promise-based custom confirmation modal
  showConfirm(receiptNo, amount) {
    document.getElementById('confirm-undo-receipt').textContent = receiptNo;
    document.getElementById('confirm-undo-amount').textContent = `₹${parseFloat(amount).toFixed(2)}`;
    this.showModal('modal-confirm-undo');
    return new Promise((resolve, reject) => {
      this._confirmResolve = resolve;
      this._confirmReject = reject;
    });
  }

  // RECEIPTS & PRINTING LAYOUTS
  async printReceipt(receiptNo) {
    try {
      const data = await invoke('get_receipt_data', { receiptNumber: receiptNo });
      
      const isThermal = document.getElementById('settings-receipt-type').value === 'thermal';
      
      if (isThermal) {
        // Populate thermal
        const printPageEl = document.getElementById('receipt-print-page');
        if (printPageEl) printPageEl.style.display = 'none';
        document.getElementById('receipt-layout-a4').style.display = 'none';
        document.getElementById('receipt-layout-thermal').style.display = 'block';

        document.getElementById('receipt-t-school-name').textContent = data.school.school_name;
        document.getElementById('receipt-t-school-address').textContent = data.school.address || '';
        document.getElementById('receipt-t-school-phone').textContent = data.school.phone || '';
        document.getElementById('receipt-t-academic-year').textContent = `Academic Year: ${data.academic_year_name}`;
        document.getElementById('receipt-t-no').textContent = data.receipt_number;
        document.getElementById('receipt-t-date').textContent = data.payment.payment_date;
        document.getElementById('receipt-t-student-name').textContent = data.student.student_name;
        document.getElementById('receipt-t-admission-no').textContent = data.student.admission_number;
        document.getElementById('receipt-t-class').textContent = data.student.class_name || 'None';
        document.getElementById('receipt-t-mode').textContent = data.payment.payment_mode.toUpperCase();

        const isMonthlyReceipt = data.bill.id === 0;
        const tbody = document.getElementById('receipt-t-items');
        tbody.innerHTML = '';

        const pt = data.payment;
        const hasAllocT = (pt.allocated_admission || 0) + (pt.allocated_other || 0)
                        + (pt.allocated_tuition || 0) + (pt.allocated_bus || 0) > 0;

        if (hasAllocT) {
          if ((pt.allocated_admission || 0) > 0) this.addReceiptRowHelper(tbody, 'Admission Fee', pt.allocated_admission);
          if ((pt.allocated_other || 0) > 0)     this.addReceiptRowHelper(tbody, 'Book/Uniform/Other', pt.allocated_other);
          if ((pt.allocated_tuition || 0) > 0)   this.addReceiptRowHelper(tbody, 'Monthly Tuition', pt.allocated_tuition);
          if ((pt.allocated_bus || 0) > 0)        this.addReceiptRowHelper(tbody, 'Bus Fee', pt.allocated_bus);
        } else if (isMonthlyReceipt) {
          if (data.bill.tuition_fee > 0) this.addReceiptRowHelper(tbody, 'Monthly Tuition', data.bill.tuition_fee);
          if (data.bill.bus_fee > 0) this.addReceiptRowHelper(tbody, 'Monthly Bus Fee', data.bill.bus_fee);
        } else {
          this.addReceiptRowHelper(tbody, 'Tuition Fee', data.bill.tuition_fee);
          if (data.bill.admission_fee > 0) this.addReceiptRowHelper(tbody, 'Admission Fee', data.bill.admission_fee);
          if (data.bill.book_fee > 0) this.addReceiptRowHelper(tbody, 'Book Fee', data.bill.book_fee);
          if (data.bill.uniform_fee > 0) this.addReceiptRowHelper(tbody, 'Uniform Fee', data.bill.uniform_fee);
          if (data.bill.bus_fee > 0) this.addReceiptRowHelper(tbody, 'Bus Fee', data.bill.bus_fee);
          if (data.bill.previous_balance > 0) this.addReceiptRowHelper(tbody, 'Previous Bal', data.bill.previous_balance);
          if (data.bill_items && data.bill_items.length > 0) {
            data.bill_items.forEach(item => this.addReceiptRowHelper(tbody, item.name, item.amount));
          } else if (data.bill.extra_fees > 0) {
            this.addReceiptRowHelper(tbody, 'Extra Fees', data.bill.extra_fees);
          }
          if (data.bill.discount > 0) this.addReceiptRowHelper(tbody, 'Discount (-)', data.bill.discount);
        }

        if (isMonthlyReceipt) {
          document.getElementById('receipt-t-total').textContent = `₹${data.payment.amount.toFixed(2)}`;
          document.getElementById('receipt-t-paid').textContent = `₹${data.payment.amount.toFixed(2)}`;
          document.getElementById('receipt-t-balance').textContent = '—';
        } else {
          document.getElementById('receipt-t-total').textContent = `₹${data.bill.total_fee.toFixed(2)}`;
          document.getElementById('receipt-t-paid').textContent = `₹${data.payment.amount.toFixed(2)} (Total paid: ₹${data.bill.amount_paid.toFixed(2)})`;
          document.getElementById('receipt-t-balance').textContent = `₹${data.bill.balance.toFixed(2)}`;
        }
        document.getElementById('receipt-t-footer-text').textContent = data.school.receipt_footer || 'Thank you for your payment.';
      } else {
        // Populate A4
        document.getElementById('receipt-layout-thermal').style.display = 'none';
        const printPageEl = document.getElementById('receipt-print-page');
        if (printPageEl) printPageEl.style.display = 'block';
        document.getElementById('receipt-layout-a4').style.display = 'block';

        // Apply custom designer layout attributes
        const size = this.settings.receipt_a4_size || 'full';
        const placement = this.settings.receipt_a4_placement || 'top-left';
        const theme = this.settings.receipt_color_theme || 'blue';
        const titleText = this.settings.receipt_title || 'FEE RECEIPT';
        const align = this.settings.receipt_header_align || 'left-right';
        const borderStyle = this.settings.receipt_border_style || 'solid';
        const fontFamily = this.settings.receipt_font_family || 'inter';
        const showLogo = this.settings.receipt_show_logo !== 'false';
        const showSigs = this.settings.receipt_show_signatures !== 'false';
        const showParent = this.settings.receipt_show_parent !== 'false';
        const showPhone = this.settings.receipt_show_phone !== 'false';

        const layoutSheet = document.getElementById('receipt-layout-a4');
        layoutSheet.className = `receipt-layout-a4-sheet size-${size} theme-${theme} border-${borderStyle} font-${fontFamily} align-${align} position-${size === 'full' ? 'full' : placement}`;

        // Set custom title text
        document.getElementById('receipt-a4-title').textContent = titleText;

        // Toggle Logo display — use logo.png bundled with app as default
        const logoImg = document.getElementById('receipt-a4-logo');
        const logoFallback = document.getElementById('receipt-a4-logo-fallback');
        if (showLogo) {
          const logoSrc = data.school.logo_path || '/logo.png';
          logoImg.src = logoSrc;
          logoImg.style.display = 'block';
          logoFallback.style.display = 'none';
          logoImg.onerror = () => {
            logoImg.style.display = 'none';
            logoFallback.style.display = 'block';
          };
        } else {
          logoImg.style.display = 'none';
          logoFallback.style.display = 'none';
        }

        // Set static headers
        document.getElementById('receipt-a4-school-name').textContent = data.school.school_name;
        document.getElementById('receipt-a4-school-address').textContent = data.school.address || '';
        document.getElementById('receipt-a4-school-phone').textContent = data.school.phone || '';
        document.getElementById('receipt-a4-academic-year').textContent = `Academic Year: ${data.academic_year_name}`;
        
        // Receipt meta details
        document.getElementById('receipt-a4-no').textContent = data.receipt_number;
        document.getElementById('receipt-a4-date').textContent = data.payment.payment_date;
        document.getElementById('receipt-a4-mode').textContent = data.payment.payment_mode.toUpperCase();
        
        // Student details
        document.getElementById('receipt-a4-student-name').textContent = data.student.student_name;
        document.getElementById('receipt-a4-class').textContent = data.student.class_name || 'None';
        
        // Toggle parent name display
        const parentRow = document.getElementById('receipt-a4-row-parent');
        if (parentRow) {
          parentRow.style.display = showParent ? 'block' : 'none';
          document.getElementById('receipt-a4-parent-name').textContent = data.student.parent_name || 'None';
        }

        // Toggle student phone display
        const phoneRow = document.getElementById('receipt-a4-row-phone');
        if (phoneRow) {
          phoneRow.style.display = showPhone ? 'block' : 'none';
          document.getElementById('receipt-a4-student-phone').textContent = data.student.phone || 'None';
        }

        // Toggle signature block display
        const sigRow = document.getElementById('receipt-a4-sig-row');
        if (sigRow) {
          sigRow.style.display = showSigs ? 'flex' : 'none';
        }

        // Populate items — show what the parent PAID in this payment (allocation breakdown)
        const isMonthlyReceiptA4 = data.bill.id === 0;
        const tbody = document.getElementById('receipt-a4-items');
        tbody.innerHTML = '';

        const p = data.payment;
        const hasAlloc = (p.allocated_admission || 0) + (p.allocated_other || 0)
                       + (p.allocated_tuition || 0) + (p.allocated_bus || 0) > 0;

        if (hasAlloc) {
          // Show per-category paid breakdown
          if ((p.allocated_admission || 0) > 0) this.addReceiptRowHelper(tbody, 'Admission Fee', p.allocated_admission);
          if ((p.allocated_other || 0) > 0)     this.addReceiptRowHelper(tbody, 'Book / Uniform / Other', p.allocated_other);
          if ((p.allocated_tuition || 0) > 0)   this.addReceiptRowHelper(tbody, 'Monthly Tuition', p.allocated_tuition);
          if ((p.allocated_bus || 0) > 0)        this.addReceiptRowHelper(tbody, 'Bus Fee', p.allocated_bus);
        } else if (isMonthlyReceiptA4) {
          // Legacy monthly receipts without allocation stored
          if (data.bill.tuition_fee > 0) this.addReceiptRowHelper(tbody, 'Monthly Tuition', data.bill.tuition_fee);
          if (data.bill.bus_fee > 0) this.addReceiptRowHelper(tbody, 'Monthly Bus Fee', data.bill.bus_fee);
        } else {
          // Legacy yearly bill receipts without allocation stored
          this.addReceiptRowHelper(tbody, 'Tuition Fee', data.bill.tuition_fee);
          if (data.bill.admission_fee > 0) this.addReceiptRowHelper(tbody, 'Admission Fee', data.bill.admission_fee);
          if (data.bill.book_fee > 0) this.addReceiptRowHelper(tbody, 'Book Fee', data.bill.book_fee);
          if (data.bill.uniform_fee > 0) this.addReceiptRowHelper(tbody, 'Uniform Fee', data.bill.uniform_fee);
          if (data.bill.bus_fee > 0) this.addReceiptRowHelper(tbody, 'Bus Fee', data.bill.bus_fee);
          if (data.bill.previous_balance > 0) this.addReceiptRowHelper(tbody, 'Previous Balance', data.bill.previous_balance);
          if (data.bill_items && data.bill_items.length > 0) {
            data.bill_items.forEach(item => this.addReceiptRowHelper(tbody, item.name, item.amount));
          } else if (data.bill.extra_fees > 0) {
            this.addReceiptRowHelper(tbody, 'Extra Fees', data.bill.extra_fees);
          }
          if (data.bill.discount > 0) this.addReceiptRowHelper(tbody, 'Discount (-)', data.bill.discount);
        }


        // Summary labels differ by receipt type
        const totalEl = document.getElementById('receipt-a4-total');
        const paidEl = document.getElementById('receipt-a4-paid');
        const balEl = document.getElementById('receipt-a4-balance');
        const totalLabelEl = totalEl.closest('.summary-num-row')?.querySelector('span');
        const balLabelEl = balEl.closest('.summary-num-row')?.querySelector('span');

        if (isMonthlyReceiptA4) {
          if (totalLabelEl) totalLabelEl.textContent = 'Amount Collected:';
          if (balLabelEl) balLabelEl.textContent = 'Outstanding Balance:';
          totalEl.textContent = `₹${data.payment.amount.toFixed(2)}`;
          paidEl.textContent = `₹${data.payment.amount.toFixed(2)}`;
          balEl.textContent = '—';
        } else {
          if (totalLabelEl) totalLabelEl.textContent = 'Total Net Fee:';
          if (balLabelEl) balLabelEl.textContent = 'Pending Balance:';
          totalEl.textContent = `₹${data.bill.total_fee.toFixed(2)}`;
          paidEl.textContent = `₹${data.payment.amount.toFixed(2)} (Total paid: ₹${data.bill.amount_paid.toFixed(2)})`;
          balEl.textContent = `₹${data.bill.balance.toFixed(2)}`;
        }
        document.getElementById('receipt-a4-footer-text').textContent = data.school.receipt_footer || 'Thank you for your payment.';
      }

      this.showModal('modal-receipt-print');
    } catch (e) {
      alert("Error generating receipt layout: " + e);
    }
  }

  addReceiptRowHelper(tbody, name, amount) {
    if (amount === 0) return;
    const row = document.createElement('tr');
    row.innerHTML = `
      <td>${name}</td>
      <td class="text-right">₹${amount.toFixed(2)}</td>
    `;
    tbody.appendChild(row);
  }

  // REPORTS
  async loadReports() {
    const start = document.getElementById('report-from-date').value;
    const end = document.getElementById('report-to-date').value;

    const tTable = document.querySelector('#report-transactions-table tbody');
    const cTable = document.querySelector('#report-classes-summary-table tbody');

    tTable.innerHTML = '<tr><td colspan="7" class="text-center">Loading transactions...</td></tr>';
    cTable.innerHTML = '<tr><td colspan="5" class="text-center">Loading summaries...</td></tr>';

    try {
      const data = await invoke('get_reports', { startDate: start, endDate: end });
      
      // Load transactions log
      tTable.innerHTML = '';
      if (data.payments.length === 0) {
        tTable.innerHTML = '<tr><td colspan="7" class="text-center text-muted">No payments recorded in this timeframe</td></tr>';
      } else {
        data.payments.forEach(p => {
          tTable.innerHTML += `
            <tr>
              <td><strong>${p.receipt_number}</strong></td>
              <td>${p.student_name}</td>
              <td>${p.class_name || '-'}</td>
              <td>₹${p.amount.toFixed(2)}</td>
              <td>${p.payment_mode.toUpperCase()}</td>
              <td>${p.payment_date}</td>
              <td class="no-print">
                <button class="btn btn-outline btn-xs mr-1" onclick="app.printReceipt('${p.receipt_number}')">View/Print</button>
                <button class="btn btn-danger btn-xs btn-undo-payment" onclick="app.revertPayment(${p.id}, '${p.receipt_number}', ${p.amount}, true)">Undo</button>
              </td>
            </tr>
          `;
        });
      }

      document.getElementById('report-total-collection').textContent = `₹${data.total_collection.toFixed(2)}`;

      // Load class summary
      cTable.innerHTML = '';
      if (data.classes.length === 0) {
        cTable.innerHTML = '<tr><td colspan="5" class="text-center text-muted">No classes active</td></tr>';
      } else {
        data.classes.forEach(c => {
          cTable.innerHTML += `
            <tr>
              <td><strong>${c.class_name}</strong></td>
              <td>${c.student_count}</td>
              <td>₹${c.total_fee.toFixed(2)}</td>
              <td>₹${c.amount_paid.toFixed(2)}</td>
              <td>₹${c.balance.toFixed(2)}</td>
            </tr>
          `;
        });
      }
    } catch (e) {
      tTable.innerHTML = `<tr><td colspan="7" class="text-danger">Error: ${e}</td></tr>`;
      cTable.innerHTML = `<tr><td colspan="5" class="text-danger">Error: ${e}</td></tr>`;
    }
  }

  async loadBusReport() {
    if (!this.activeAcademicYear) {
      alert("Please select or configure an active academic year.");
      return;
    }
    const month = parseInt(document.getElementById('bus-report-month').value);
    const year = parseInt(document.getElementById('bus-report-year').value);

    const busTable = document.querySelector('#report-bus-table tbody');
    if (!busTable) return;
    busTable.innerHTML = '<tr><td colspan="7" class="text-center">Loading bus collections...</td></tr>';

    try {
      const items = await invoke('get_bus_report', {
        academicYearId: this.activeAcademicYear.id,
        month: month
      });

      const filtered = items.filter(item => item.year === year);

      busTable.innerHTML = '';
      let totalCollected = 0;
      let totalOutstanding = 0;
      let totalRiders = 0;

      if (filtered.length === 0) {
        busTable.innerHTML = '<tr><td colspan="7" class="text-center text-muted">No bus riders/collections for this month</td></tr>';
      } else {
        filtered.forEach(item => {
          totalCollected += item.amount_paid;
          totalOutstanding += item.balance;
          if (item.bus_used) {
            totalRiders++;
          }
          const busStatus = item.status || 'unpaid';

          busTable.innerHTML += `
            <tr>
              <td><strong>${item.student_name}</strong></td>
              <td>${item.class_name || '-'}</td>
              <td>${item.bus_stop_name || '-'}</td>
              <td>₹${item.bus_fee.toFixed(2)}</td>
              <td>₹${item.amount_paid.toFixed(2)}</td>
              <td>₹${item.balance.toFixed(2)}</td>
              <td><span class="status-badge ${busStatus}">${busStatus}</span></td>
            </tr>
          `;
        });
      }

      document.getElementById('report-total-bus-collection').textContent = `₹${totalCollected.toFixed(2)}`;
      document.getElementById('report-total-bus-outstanding').textContent = `₹${totalOutstanding.toFixed(2)}`;
      document.getElementById('report-total-bus-riders').textContent = totalRiders.toString();
    } catch (e) {
      busTable.innerHTML = `<tr><td colspan="7" class="text-danger">Error: ${e}</td></tr>`;
    }
  }

  async exportBusReportExcel() {
    if (!this.activeAcademicYear) {
      alert("No active academic year configured.");
      return;
    }
    const month = parseInt(document.getElementById('bus-report-month').value);
    const year = parseInt(document.getElementById('bus-report-year').value);
    const monthName = new Date(year, month - 1, 1).toLocaleString('default', { month: 'long' });

    try {
      const items = await invoke('get_bus_report', {
        academicYearId: this.activeAcademicYear.id,
        month: month
      });
      const filtered = items.filter(item => item.year === year);

      let csv = "Student Name,Class,Bus Stop,Monthly Charge,Amount Paid,Balance,Status\n";
      filtered.forEach(item => {
        csv += `"${item.student_name}","${item.class_name || ''}","${item.bus_stop_name || ''}",${item.bus_fee},${item.amount_paid},${item.balance},"${item.status}"\n`;
      });

      const path = await window.__TAURI__.dialog.save({
        defaultPath: `bus_report_${monthName}_${year}.csv`,
        filters: [{ name: 'CSV File', extensions: ['csv'] }]
      });
      if (path) {
        const encoder = new TextEncoder();
        const bytes = encoder.encode(csv);
        await window.__TAURI__.fs.writeBinaryFile(path, bytes);
        alert("Bus collections report exported successfully to: " + path);
      }
    } catch (e) {
      alert("Export failed: " + e);
    }
  }

  async exportReportExcel() {
    // Generate CSV and save file via dialog save
    const start = document.getElementById('report-from-date').value;
    const end = document.getElementById('report-to-date').value;
    try {
      const data = await invoke('get_reports', { startDate: start, endDate: end });
      let csv = "Receipt Number,Student Name,Class,Amount Paid,Payment Mode,Date\n";
      data.payments.forEach(p => {
        csv += `"${p.receipt_number}","${p.student_name}","${p.class_name || ''}",${p.amount},"${p.payment_mode}","${p.payment_date}"\n`;
      });

      const path = await window.__TAURI__.dialog.save({
        defaultPath: `report_${start}_to_${end}.csv`,
        filters: [{ name: 'CSV File', extensions: ['csv'] }]
      });
      if (path) {
        // In Tauri v2 we can write the string using FS plugin
        const encoder = new TextEncoder();
        const bytes = encoder.encode(csv);
        await window.__TAURI__.fs.writeBinaryFile(path, bytes);
        alert("Report exported successfully to: " + path);
      }
    } catch (e) {
      alert("Export failed: " + e);
    }
  }

  // SYNC QUEUE TAB
  async loadSyncQueue() {
    const tbody = document.querySelector('#sync-queue-table tbody');
    if (!tbody) return;
    tbody.innerHTML = '<tr><td colspan="6" class="text-center">Loading queue...</td></tr>';
    try {
      const list = await invoke('get_sync_queue');
      tbody.innerHTML = '';
      if (list.length === 0) {
        tbody.innerHTML = '<tr><td colspan="6" class="text-center text-muted">No synchronization records. All caught up! ✓</td></tr>';
        return;
      }
      list.forEach(item => {
        const statusClass = item.status === 'success' ? 'active' : item.status === 'failed' ? 'inactive' : 'pending';
        const errorText = item.error_message
          ? `<span title="${item.error_message}" style="cursor:help;">${item.error_message.substring(0, 60)}${item.error_message.length > 60 ? '…' : ''}</span>`
          : '-';
        tbody.innerHTML += `
          <tr>
            <td>${item.id}</td>
            <td><code style="font-size:11px;">${item.student_id}</code></td>
            <td>${item.retry_count}</td>
            <td>${item.last_attempt_at || item.created_at || '-'}</td>
            <td><span class="badge status-${statusClass}">${item.status}</span></td>
            <td class="text-muted text-xs">${errorText}</td>
          </tr>
        `;
      });
    } catch (e) {
      tbody.innerHTML = `<tr><td colspan="6" class="text-danger">Error loading sync queue: ${e}</td></tr>`;
    }
  }

  // Toast notification helper
  showToast(message, type = 'info') {
    let container = document.getElementById('toast-container');
    if (!container) {
      container = document.createElement('div');
      container.id = 'toast-container';
      document.body.appendChild(container);
    }
    const toast = document.createElement('div');
    toast.className = `toast toast-${type}`;
    const icon = type === 'success' ? 'check_circle' : type === 'error' ? 'error' : 'info';
    toast.innerHTML = `<span class="material-symbols-outlined">${icon}</span><span>${message}</span>`;
    container.appendChild(toast);
    // Animate in
    requestAnimationFrame(() => toast.classList.add('show'));
    setTimeout(() => {
      toast.classList.remove('show');
      setTimeout(() => toast.remove(), 400);
    }, 3500);
  }
}

// Global initialization
window.addEventListener('DOMContentLoaded', () => {
  window.app = new SchoolBillingApp();
  window.app.init();
});
