use super::login::{LoginEmailCommand, LoginEmailHandler, LoginError, LoginResult};
use tokio::sync::{mpsc, oneshot};

/// Wraps an auth command with its response channel.
#[derive(Debug)]
pub enum AuthCommand {
    LoginEmail(LoginEmailCommand, oneshot::Sender<Result<LoginResult, LoginError>>),
}

/// Handle to send commands to the mediator. Clones of this can be shared.
#[derive(Clone)]
pub struct MediatorHandle {
    tx: mpsc::Sender<AuthCommand>,
}

impl MediatorHandle {
    pub fn new(tx: mpsc::Sender<AuthCommand>) -> Self {
        Self { tx }
    }

    /// Sends a login command and awaits the result via oneshot.
    pub async fn send_login(
        &self,
        cmd: LoginEmailCommand,
    ) -> Result<LoginResult, LoginError> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.tx
            .send(AuthCommand::LoginEmail(cmd, resp_tx))
            .await
            .map_err(|_| LoginError::InternalError("Mediator channel closed".to_string()))?;
        resp_rx
            .await
            .map_err(|_| LoginError::InternalError("Mediator dropped response".to_string()))?
    }
}

/// Background worker that receives commands and dispatches to handlers.
pub struct MediatorWorker {
    rx: mpsc::Receiver<AuthCommand>,
    login_handler: LoginEmailHandler,
}

impl MediatorWorker {
    pub fn new(rx: mpsc::Receiver<AuthCommand>, login_handler: LoginEmailHandler) -> Self {
        Self {
            rx,
            login_handler,
        }
    }

    /// Run the dispatch loop. Returns when the channel is closed.
    pub async fn run(mut self) {
        while let Some(cmd) = self.rx.recv().await {
            match cmd {
                AuthCommand::LoginEmail(command, resp_tx) => {
                    let result = self.login_handler.execute(command).await;
                    let _ = resp_tx.send(result);
                }
            }
        }
    }
}

/// Builds a mediator with a handle and a worker ready to be spawned.
pub fn create_mediator(
    login_handler: LoginEmailHandler,
    buffer: usize,
) -> (MediatorHandle, MediatorWorker) {
    let (tx, rx) = mpsc::channel(buffer);
    let handle = MediatorHandle::new(tx);
    let worker = MediatorWorker::new(rx, login_handler);
    (handle, worker)
}
