use std::time::Duration;
use tokio::sync::watch;

pub async fn signal() {
    let interrupt = async {
        tokio::signal::ctrl_c()
            .await
            .expect("installing the Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("installing the SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = interrupt => {}
        _ = terminate => {}
    }
}

#[derive(Clone)]
pub struct Shutdown(watch::Receiver<bool>);

impl Shutdown {
    pub fn channel() -> (watch::Sender<bool>, Self) {
        let (sender, receiver) = watch::channel(false);
        (sender, Self(receiver))
    }

    pub fn is_triggered(&self) -> bool {
        *self.0.borrow()
    }

    pub async fn sleep(&mut self, duration: Duration) -> bool {
        if self.is_triggered() {
            return false;
        }

        tokio::select! {
            _ = tokio::time::sleep(duration) => true,
            _ = self.0.changed() => !self.is_triggered(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn sleeps_for_the_full_duration_when_nothing_happens() {
        let (_sender, mut shutdown) = Shutdown::channel();
        assert!(shutdown.sleep(Duration::from_secs(30)).await);
    }

    #[tokio::test(start_paused = true)]
    async fn wakes_early_when_shutdown_fires() {
        let (sender, mut shutdown) = Shutdown::channel();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            sender.send(true).unwrap();
        });

        assert!(!shutdown.sleep(Duration::from_secs(3600)).await);
    }

    #[tokio::test]
    async fn refuses_to_sleep_once_triggered() {
        let (sender, mut shutdown) = Shutdown::channel();
        sender.send(true).unwrap();

        assert!(shutdown.is_triggered());
        assert!(!shutdown.sleep(Duration::from_secs(3600)).await);
    }
}
