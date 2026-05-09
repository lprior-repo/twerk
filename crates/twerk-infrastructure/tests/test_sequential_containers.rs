#![allow(clippy::unwrap_used)]

use testcontainers::runners::AsyncRunner;
use testcontainers::ImageExt;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::rabbitmq::RabbitMq;

#[tokio::test]
async fn test_sequential_postgres_then_rabbitmq() {
    // Start postgres first (like distributed test does)
    let pg = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .unwrap();
    let pg_port = pg.get_host_port_ipv4(5432).await.unwrap();
    println!("Postgres port: {}", pg_port);

    // Then start rabbitmq
    let rabbit = RabbitMq::default().start().await.unwrap();
    let rabbit_port = rabbit.get_host_port_ipv4(5672).await.unwrap();
    println!("RabbitMQ port: {}", rabbit_port);

    // Try to connect like the coordinator does
    let url = format!("amqp://guest:guest@127.0.0.1:{}", rabbit_port);
    let conn = lapin::Connection::connect(&url, lapin::ConnectionProperties::default()).await;
    match conn {
        Ok(_) => println!("Connected successfully"),
        Err(e) => println!("Connection failed: {}", e),
    }
}

#[tokio::test]
async fn test_multiple_engines_with_rabbitmq() {
    // Start rabbitmq
    let rabbit = RabbitMq::default().start().await.unwrap();
    let rabbit_port = rabbit.get_host_port_ipv4(5672).await.unwrap();
    let url = format!("amqp://guest:guest@127.0.0.1:{}", rabbit_port);

    // Set env var like distributed test does
    std::env::set_var("TWERK_BROKER_RABBITMQ_URL", &url);
    std::env::set_var("TWERK_BROKER_TYPE", "rabbitmq");

    // Try to connect multiple times (like broker factory does 3 connections)
    for i in 1..=3 {
        let conn = lapin::Connection::connect(&url, lapin::ConnectionProperties::default()).await;
        match conn {
            Ok(_) => println!("Connection {}: OK", i),
            Err(e) => println!("Connection {}: FAILED - {}", i, e),
        }
    }
}
