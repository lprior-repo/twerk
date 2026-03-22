//! Engine tests

use crate::engine::{Engine, Config, Mode, State};

#[tokio::test]
async fn test_engine_new() {
    let engine = Engine::new(Config::default());
    assert_eq!(engine.state(), State::Idle);
    assert_eq!(engine.mode(), Mode::Standalone);
}

#[tokio::test]
async fn test_engine_start_standalone() {
    let mut engine = Engine::new(Config {
        mode: Mode::Standalone,
    });
    
    assert_eq!(engine.state(), State::Idle);
    
    engine.start().await.expect("should start");
    
    assert_eq!(engine.state(), State::Running);
    
    engine.terminate().await.expect("should terminate");
    
    assert_eq!(engine.state(), State::Terminated);
}

#[tokio::test]
async fn test_engine_start_coordinator() {
    let mut engine = Engine::new(Config {
        mode: Mode::Coordinator,
    });
    
    assert_eq!(engine.state(), State::Idle);
    
    engine.start().await.expect("should start");
    
    assert_eq!(engine.state(), State::Running);
    
    engine.terminate().await.expect("should terminate");
    
    assert_eq!(engine.state(), State::Terminated);
}

#[tokio::test]
async fn test_engine_start_worker() {
    let mut engine = Engine::new(Config {
        mode: Mode::Worker,
    });
    
    assert_eq!(engine.state(), State::Idle);
    
    engine.start().await.expect("should start");
    
    assert_eq!(engine.state(), State::Running);
    
    engine.terminate().await.expect("should terminate");
    
    assert_eq!(engine.state(), State::Terminated);
}

#[tokio::test]
async fn test_engine_set_mode_when_idle() {
    let mut engine = Engine::new(Config::default());
    assert_eq!(engine.mode(), Mode::Standalone);
    
    engine.set_mode(Mode::Worker);
    assert_eq!(engine.mode(), Mode::Worker);
}

#[tokio::test]
async fn test_engine_set_mode_when_running() {
    let mut engine = Engine::new(Config {
        mode: Mode::Standalone,
    });
    
    engine.start().await.expect("should start");
    
    // Setting mode should not work when running
    engine.set_mode(Mode::Worker);
    // Mode should remain Standalone since state is not Idle
    assert_eq!(engine.mode(), Mode::Standalone);
    
    engine.terminate().await.expect("should terminate");
}

#[tokio::test]
async fn test_engine_double_start() {
    let mut engine = Engine::new(Config::default());
    
    engine.start().await.expect("should start");
    
    let result = engine.start().await;
    assert!(result.is_err());
    
    engine.terminate().await.expect("should terminate");
}

#[tokio::test]
async fn test_engine_terminate_when_not_running() {
    let mut engine = Engine::new(Config::default());
    
    let result = engine.terminate().await;
    assert!(result.is_err());
}
