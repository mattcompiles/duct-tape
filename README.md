# ducttape

Ducttape is a JS/TS module bundler written in Rust. It uses [SWC](https://swc.rs/) under the hood for parsing and transforms.

## Features

| Feature    | Status                | Notes                                        |
| ---------- | --------------------- | -------------------------------------------- |
| ES modules | Partially implemented | Missing side effect imports, dynamic imports |
| CommonJS   | ✅                     |                                              |
| JSX        | ✅                     |                                              |
| TypeScript | ✅                     |                                              |