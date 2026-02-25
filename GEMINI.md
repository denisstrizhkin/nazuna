# Nazuna Instruction Manual

You are acting as a **Senior Rust Engineer** maintaining the Nazuna WireGuard management tool. Nazuna is designed to be a lightweight, data-driven, and modular tool. Adhere to the following permanent instructions:

## 1. Architectural Philosophy
- **Senior Rust Standards**: Prioritize safety, performance, and readability. Use idiomatic Rust patterns (e.g., proper trait usage, encapsulation, and functional iterators).
- **DRY (Don't Repeat Yourself)**: Always unify repetitive logic. For example, if you see multiple `Command::new` calls for the same binary, abstract them into a helper.
- **Data Model Integrity**: The `users.json` file is the absolute source of truth. Ensure that any modification to system state is backed by an update to the database.

## 2. Error Handling Protocol
- **Anyhow Only**: Use `anyhow::Result` for all top-level and complex internal functions. Avoid `unwrap()` or `expect()` unless it's in a test or a truly impossible path.
- **Context is King**: Always use `.context()` or `.with_context()` to provide diagnostic information. 
    - *Poor*: `fs::read_to_string(path)?`
    - *Proper*: `fs::read_to_string(path).with_context(|| format!("Failed to read database at {}", path))?`
- **Binary Failures**: When calling external binaries like `wg` or `wg-quick`, capture and return the `stderr` content in the error message to aid debugging.

## 3. Documentation Synchronicity
- **README.md Maintenance**: The `README.md` must be a high-fidelity representation of the current CLI interface.
- **Automatic Updates**: Every time you add a new command, change a flag, or modify an environment variable requirement, you **MUST** update the `README.md` to reflect these changes before finalizing the task.

## 4. Subnet & Network Logic
- **ipnet Integration**: Use the `ipnet` crate for all CIDR and IP math. Never perform manual octet manipulation.
- **Environment First**: Network parameters (like endpoints and server IPs) should be pulled from the environment (`WgEnv`) to allow Nazis to remain portable.

## 5. Development Cycle
1. **Plan**: Identify DRY opportunities.
2. **Implement**: Rust source changes first.
3. **Verify**: Always run the following checks after modifications to ensure stability and code quality:
    - `cargo check` (Fast validation)
    - `cargo fmt` (Consitent style)
    - `cargo clippy` (Idiomatic standards)
4. **Document**: Update `README.md` and this file if project rules evolve.
