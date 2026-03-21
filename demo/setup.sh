#!/usr/bin/env bash
# Create a sample project for the trev demo.
# Usage: ./demo/setup.sh
set -euo pipefail

DIR=/tmp/trev-demo

rm -rf "$DIR"
mkdir -p "$DIR"

# --- Go module ---
mkdir -p "$DIR/cmd/server" "$DIR/internal/handler" "$DIR/internal/middleware" "$DIR/internal/handler"

cat > "$DIR/go.mod" << 'GO'
module github.com/example/webapp

go 1.23

require (
	github.com/go-chi/chi/v5 v5.0.12
	github.com/rs/zerolog v1.33.0
)
GO

cat > "$DIR/cmd/server/main.go" << 'GO'
package main

import (
	"fmt"
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/example/webapp/internal/handler"
	"github.com/example/webapp/internal/middleware"
)

func main() {
	r := chi.NewRouter()
	r.Use(middleware.Logger)
	r.Get("/", handler.Home)
	r.Get("/api/health", handler.Health)

	fmt.Println("Listening on :8080")
	http.ListenAndServe(":8080", r)
}
GO

cat > "$DIR/internal/handler/home.go" << 'GO'
package handler

import "net/http"

func Home(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "text/html")
	w.Write([]byte("<h1>Welcome</h1>"))
}
GO

cat > "$DIR/internal/handler/health.go" << 'GO'
package handler

import (
	"encoding/json"
	"net/http"
)

type HealthResponse struct {
	Status  string `json:"status"`
	Version string `json:"version"`
}

func Health(w http.ResponseWriter, r *http.Request) {
	resp := HealthResponse{Status: "ok", Version: "1.0.0"}
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(resp)
}
GO

cat > "$DIR/internal/handler/health_test.go" << 'GO'
package handler

import (
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestHealth(t *testing.T) {
	req := httptest.NewRequest("GET", "/api/health", nil)
	rec := httptest.NewRecorder()
	Health(rec, req)

	if rec.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", rec.Code)
	}
}
GO

cat > "$DIR/internal/middleware/logger.go" << 'GO'
package middleware

import (
	"net/http"
	"time"

	"github.com/rs/zerolog/log"
)

func Logger(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		start := time.Now()
		next.ServeHTTP(w, r)
		log.Info().
			Str("method", r.Method).
			Str("path", r.URL.Path).
			Dur("duration", time.Since(start)).
			Msg("request")
	})
}
GO

# --- TypeScript frontend ---
mkdir -p "$DIR/web/src/components" "$DIR/web/src/hooks" "$DIR/web/src/utils"
mkdir -p "$DIR/web/node_modules/.package-lock.json" && rm -rf "$DIR/web/node_modules/.package-lock.json"
mkdir -p "$DIR/web/node_modules/react" "$DIR/web/node_modules/next"

cat > "$DIR/web/package.json" << 'JSON'
{
  "name": "webapp-frontend",
  "version": "1.0.0",
  "scripts": {
    "dev": "next dev",
    "build": "next build",
    "test": "vitest"
  },
  "dependencies": {
    "next": "^15.0.0",
    "react": "^19.0.0",
    "react-dom": "^19.0.0"
  },
  "devDependencies": {
    "typescript": "^5.6.0",
    "vitest": "^3.0.0"
  }
}
JSON

cat > "$DIR/web/package-lock.json" << 'JSON'
{
  "name": "webapp-frontend",
  "version": "1.0.0",
  "lockfileVersion": 3,
  "requires": true,
  "packages": {
    "": {
      "name": "webapp-frontend",
      "version": "1.0.0",
      "dependencies": {
        "next": "^15.0.0",
        "react": "^19.0.0",
        "react-dom": "^19.0.0"
      },
      "devDependencies": {
        "typescript": "^5.6.0",
        "vitest": "^3.0.0"
      }
    },
    "node_modules/next": {
      "version": "15.1.6",
      "resolved": "https://registry.npmjs.org/next/-/next-15.1.6.tgz",
      "license": "MIT",
      "dependencies": {
        "@next/env": "15.1.6",
        "@swc/counter": "0.1.3"
      },
      "bin": {
        "next": "dist/bin/next"
      },
      "engines": {
        "node": ">=18.18.0"
      },
      "peerDependencies": {
        "react": "^18.2.0 || 19.0.0-rc-de68d2f4-20241204 || ^19.0.0",
        "react-dom": "^18.2.0 || 19.0.0-rc-de68d2f4-20241204 || ^19.0.0"
      }
    },
    "node_modules/react": {
      "version": "19.0.0",
      "resolved": "https://registry.npmjs.org/react/-/react-19.0.0.tgz",
      "license": "MIT",
      "engines": {
        "node": ">=0.10.0"
      }
    },
    "node_modules/react-dom": {
      "version": "19.0.0",
      "resolved": "https://registry.npmjs.org/react-dom/-/react-dom-19.0.0.tgz",
      "license": "MIT",
      "dependencies": {
        "scheduler": "^0.25.0"
      },
      "peerDependencies": {
        "react": "^19.0.0"
      }
    },
    "node_modules/typescript": {
      "version": "5.7.3",
      "resolved": "https://registry.npmjs.org/typescript/-/typescript-5.7.3.tgz",
      "dev": true,
      "license": "Apache-2.0",
      "bin": {
        "tsc": "bin/tsc",
        "tsserver": "bin/tsserver"
      },
      "engines": {
        "node": ">=14.17"
      }
    },
    "node_modules/vitest": {
      "version": "3.0.4",
      "resolved": "https://registry.npmjs.org/vitest/-/vitest-3.0.4.tgz",
      "dev": true,
      "license": "MIT",
      "bin": {
        "vitest": "vitest.mjs"
      },
      "engines": {
        "node": "^18.0.0 || ^20.0.0 || >=22.0.0"
      }
    }
  }
}
JSON

cat > "$DIR/web/tsconfig.json" << 'JSON'
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "jsx": "react-jsx",
    "strict": true,
    "baseUrl": ".",
    "paths": { "@/*": ["src/*"] }
  }
}
JSON

cat > "$DIR/web/src/components/Button.tsx" << 'TSX'
import { type ReactNode } from "react";

interface ButtonProps {
  children: ReactNode;
  variant?: "primary" | "secondary";
  onClick?: () => void;
}

export function Button({ children, variant = "primary", onClick }: ButtonProps) {
  return (
    <button className={`btn btn-${variant}`} onClick={onClick}>
      {children}
    </button>
  );
}
TSX

cat > "$DIR/web/src/components/Button.test.tsx" << 'TSX'
import { describe, it, expect } from "vitest";
import { Button } from "./Button";

describe("Button", () => {
  it("renders children", () => {
    // placeholder test
    expect(true).toBe(true);
  });
});
TSX

cat > "$DIR/web/src/components/Header.tsx" << 'TSX'
import { Button } from "./Button";

export function Header() {
  return (
    <header className="header">
      <h1>WebApp</h1>
      <nav>
        <Button variant="secondary" onClick={() => {}}>
          Login
        </Button>
      </nav>
    </header>
  );
}
TSX

cat > "$DIR/web/src/hooks/useAuth.ts" << 'TS'
import { useState, useCallback } from "react";

interface User {
  id: string;
  name: string;
  email: string;
}

export function useAuth() {
  const [user, setUser] = useState<User | null>(null);

  const login = useCallback(async (email: string, password: string) => {
    const res = await fetch("/api/auth/login", {
      method: "POST",
      body: JSON.stringify({ email, password }),
    });
    const data = await res.json();
    setUser(data.user);
  }, []);

  const logout = useCallback(() => setUser(null), []);

  return { user, login, logout };
}
TS

cat > "$DIR/web/src/utils/format.ts" << 'TS'
export function formatDate(date: Date): string {
  return new Intl.DateTimeFormat("en-US", {
    year: "numeric",
    month: "short",
    day: "numeric",
  }).format(date);
}

export function formatBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB"];
  let i = 0;
  let value = bytes;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i++;
  }
  return `${value.toFixed(1)} ${units[i]}`;
}
TS

cat > "$DIR/web/src/utils/format.test.ts" << 'TS'
import { describe, it, expect } from "vitest";
import { formatBytes } from "./format";

describe("formatBytes", () => {
  it("formats bytes", () => {
    expect(formatBytes(1024)).toBe("1.0 KB");
  });

  it("formats megabytes", () => {
    expect(formatBytes(1048576)).toBe("1.0 MB");
  });
});
TS

# Placeholder node_modules content (just enough for gitignore demo)
echo '{"name":"react"}' > "$DIR/web/node_modules/react/package.json"
echo '{"name":"next"}' > "$DIR/web/node_modules/next/package.json"

# --- Root files ---
cat > "$DIR/README.md" << 'MD'
# webapp

A full-stack web application with Go backend and Next.js frontend.

## Getting Started

```sh
# Backend
go run cmd/server/main.go

# Frontend
cd web && npm run dev
```

## Architecture

```
cmd/server/     → HTTP server entry point
internal/       → Business logic & middleware
web/            → Next.js frontend
```
MD

cat > "$DIR/Makefile" << 'MAKE'
.PHONY: dev build test

dev:
	@echo "Starting backend..."
	go run cmd/server/main.go &
	@echo "Starting frontend..."
	cd web && npm run dev

build:
	go build -o bin/server cmd/server/main.go
	cd web && npm run build

test:
	go test ./...
	cd web && npm test
MAKE

cat > "$DIR/LICENSE" << 'TXT'
MIT License

Copyright (c) 2025 example
TXT

cat > "$DIR/.gitignore" << 'TXT'
bin/
node_modules/
.next/
*.exe
TXT

# Set realistic modification times (relative to now).
# macOS touch doesn't support -d, so we use date -v to compute timestamps.
ago() { touch -t "$(date -v"$1" +%Y%m%d%H%M.%S)" "$2"; }

# Source files
ago -1H   "$DIR/web/src/components/Button.tsx"
ago -3H   "$DIR/web/src/hooks/useAuth.ts"
ago -5H   "$DIR/web/src/utils/format.ts"
ago -8H   "$DIR/web/src/components/Header.tsx"
ago -1d   "$DIR/internal/handler/health.go"
ago -2d   "$DIR/internal/handler/home.go"
ago -5d   "$DIR/internal/middleware/logger.go"
ago -7d   "$DIR/cmd/server/main.go"

# Config / meta files
ago -10d  "$DIR/web/package.json"
ago -10d  "$DIR/web/package-lock.json"
ago -10d  "$DIR/web/tsconfig.json"
ago -14d  "$DIR/go.mod"
ago -21d  "$DIR/.gitignore"
ago -30d  "$DIR/README.md"
ago -60d  "$DIR/Makefile"
ago -90d  "$DIR/LICENSE"

# Test files — slightly older than their source
ago -4H   "$DIR/web/src/components/Button.test.tsx"
ago -6H   "$DIR/web/src/utils/format.test.ts"
ago -3d   "$DIR/internal/handler/health_test.go"

# node_modules — old
ago -10d  "$DIR/web/node_modules/react/package.json"
ago -10d  "$DIR/web/node_modules/next/package.json"

# Directories — match the newest file inside each
ago -1H   "$DIR/web/src/components"
ago -3H   "$DIR/web/src/hooks"
ago -5H   "$DIR/web/src/utils"
ago -1H   "$DIR/web/src"
ago -1H   "$DIR/web"
ago -7d   "$DIR/cmd/server"
ago -7d   "$DIR/cmd"
ago -1d   "$DIR/internal/handler"
ago -5d   "$DIR/internal/middleware"
ago -1d   "$DIR/internal"
ago -10d  "$DIR/web/node_modules"
ago -10d  "$DIR/web/node_modules/react"
ago -10d  "$DIR/web/node_modules/next"

# Initialize git repo for gitignore to work
cd "$DIR" && git init -q && git add -A && git commit -q -m "init"

echo "Demo project created at $DIR"
