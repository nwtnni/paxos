pub struct Server(std::process::Child);

impl Server {
    pub fn new(
        path: &std::path::PathBuf,
        id: usize,
        port: usize,
        count: usize,
        verbose: u8,
    ) -> Self {
        let id = id.to_string();
        let port = port.to_string();
        let count = count.to_string();
        let mut command = std::process::Command::new(path);
        if verbose > 0 {
            let verbosity = "-".to_string() + &"v".repeat(verbose as usize);
            command.arg(&verbosity);
        }
        command.args(&["-i", &id])
            .args(&["-p", &port])
            .args(&["-c", &count])
            .spawn()
            .map(Server)
            .expect("[INTERNAL ERROR]: could not spawn server")
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.0.kill().ok();
    }
}
