use api::{AppState, AuthService, router};

use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let state = AppState {
        auth: AuthService {
            login: application::auth::login::LoginEmailHandler::new(
                persistence::SqlxUserRepository::from_pool(
                    persistence::connect(&persistence::Config::from_env().unwrap())
                        .await
                        .unwrap(),
                ),
            ),
        },
    };
    let app = router(state);
    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
