# ✨ Twerk ✨
> **Tork's fabulous Rust twin 💅**

[![Rust](https://img.shields.io/badge/Rust-1.75+-pink.svg?style=for-the-badge)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-ff69b4.svg?style=for-the-badge)](LICENSE)

---

## 💖 What is Twerk?

Twerk is the *exquisite* Rust reimagining of [Tork](https://github.com/runabol/tork) — the distributed task execution system that makes your containers dance. While Tork was written in Go (perfectly fine, we don't judge 💅), Twerk is the sassy, type-safe, blazingly fast Rust version that was *Born to Shine* ✨

Like its predecessor, Twerk lets you define workflows as tasks that run in Docker, Podman, or Shell environments. But with Rust's compiler at the helm, we catch bugs at compile time rather than letting them run wild at runtime. *That's how we do it* 🕺

---

## 🌟 Features

- **🔥 Blazingly Fast** — Rust's zero-cost abstractions mean Twerk is fast *and* safe
- **🐳 Multi-Runtime Support** — Docker, Podman, Shell — Twerk works with them all
- **📦 Distributed Execution** — Scale tasks across multiple nodes with grace
- **🛡️ Type-Safe** — The compiler is your guard, keeping bugs at bay
- **⚡ Async Everything** — Built on Tokio for maximum throughput
- **🎀 Beautiful Code** — Because you deserve to read code that sparkles

---

## 🚀 Getting Started

```bash
# Clone the repo
git clone https://github.com/lprior-repo/twerk.git
cd twerk

# Build with love
cargo build --release

# Run the fabulous CLI
cargo run --bin twerk -- help
```

---

## 📁 Project Structure

```
twerk/
├── tork/              # 💎 Core domain types — the crown jewels
├── locker/            # 🔐 Distributed locking — keeping things safe
├── engine/            # ⚙️ The orchestration engine — the heart
├── broker/            # 📨 Message broker — RabbitMQ & in-memory
├── datastore/         # 💾 Data storage — Postgres & more
├── cli/               # 🎀 Command line interface — our face to the world
├── health/            # 💊 Health checks — keeping Twerk thriving
├── input/             # 📥 Input validation — checking that everything is perfect
├── coordinator/       # 👑 Job coordinator — making sure everything runs on time
└── runtime/           # 🏃 Runtime implementations — Docker, Podman, Shell
```

---

## 💅 Code of Conduct

Twerk follows the principle: **Be kind, be helpful, be excellent to each other**. We don't have tolerance for code that doesn't treat others with respect. Pull requests that don't meet this standard will be shown the door 🚪✨

---

## 📜 License

Twerk is released under the MIT License. See [LICENSE](LICENSE) for the details, honey 💅

---

*Made with 💖 and a lot of ☕ by developers who believe code should be as fabulous as it is functional*
