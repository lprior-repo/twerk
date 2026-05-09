#[tokio::test]
async fn test_rabbitmq_container() {
    use testcontainers::runners::AsyncRunner;
    use testcontainers_modules::rabbitmq::RabbitMq;

    let container = RabbitMq::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(5672).await.unwrap();
    println!("RabbitMQ port: {}", port);

    let url = format!("amqp://guest:guest@127.0.0.1:{port}");
    let conn = lapin::Connection::connect(&url, lapin::ConnectionProperties::default()).await;
    match conn {
        Ok(_) => println!("Connected successfully"),
        Err(e) => println!("Connection failed: {}", e),
    }

    // Keep container alive for inspection
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    println!("Test complete");
}
