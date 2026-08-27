# Web 开发与部署

## 前置条件

- Rust stable
- Node.js 18+
- npm

## 开发

安装前端依赖：

```bash
cd web
npm install
```

启动后端 API：

```bash
cargo run
```

一键启动前后端并打开浏览器：

```bash
./scripts/dev-web.sh
```

启动前端开发服务器：

```bash
cd web
npm run dev
```

默认端口：

- 前端：`http://localhost:1420`
- 后端：`http://127.0.0.1:10086`，默认监听 `0.0.0.0:10086`，局域网内其他电脑可通过 `http://<本机局域网IP>:10086` 访问。

## 构建

构建后端：

```bash
cargo build --release
```

构建前端：

```bash
cd web
npm run build
```

后端会在生产模式下尝试从 `web/dist` 提供静态资源；如果没有前端构建产物，只会提供 API。
