fn get_cpu_percent() -> f64 {
    let mut sys = sysinfo::System::new_all();
    sys.refresh_cpu_all();
    std::thread::sleep(Duration::from_millis(200));
    sys.refresh_cpu_all();

    let cpus = sys.cpus();
    if cpus.is_empty() {
        return 0.0;
    }

    cpus.iter()
        .map(|cpu: &sysinfo::Cpu| cpu.cpu_usage() as f64)
        .sum::<f64>()
        / cpus.len() as f64
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
