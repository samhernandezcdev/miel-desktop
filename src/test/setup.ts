import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// Sin `globals: true`, Testing Library no limpia el DOM entre pruebas por sí sola.
afterEach(() => cleanup());

