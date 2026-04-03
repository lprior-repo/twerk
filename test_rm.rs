use testcontainers_modules::postgres::Postgres;
use testcontainers::runners::AsyncRunner;

async fn test() {
    let container = Postgres::default().with_tag("16-alpine").start().await.unwrap();
    // What methods exist?
}
