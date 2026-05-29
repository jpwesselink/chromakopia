import path from "node:path";
import { defineConfig } from "@rspress/core";

export default defineConfig({
  root: "docs",
  base: "/chromakopia/",
  title: "chromakopia",
  description: "Beautiful terminal animations for Rust",
  globalStyles: path.join(__dirname, "docs/styles/index.css"),
  themeConfig: {
    socialLinks: [
      { icon: "github", mode: "link", content: "https://github.com/jpwesselink/chromakopia" },
    ],
  },
  builderConfig: {
    tools: {
      rspack(config: any) {
        config.experiments = {
          ...config.experiments,
          asyncWebAssembly: true,
        };
        return config;
      },
    },
    source: {
      alias: {
        "@wasm": path.join(__dirname, "wasm"),
        "@components": path.join(__dirname, "components"),
      },
    },
  },
});
