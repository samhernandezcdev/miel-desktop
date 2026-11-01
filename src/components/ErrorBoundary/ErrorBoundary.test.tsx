import { render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ErrorBoundary, MAX_RETRIES } from "./ErrorBoundary";

function Boom({ explode }: { explode: boolean }) {
  if (explode) throw new Error("falla de prueba");
  return <p data-testid="ok">ok</p>;
}

let errSpy: ReturnType<typeof vi.spyOn>;
beforeEach(() => {
  // React registra el error capturado en la consola: se silencia en estas pruebas
  errSpy = vi.spyOn(console, "error").mockImplementation(() => {});
});
afterEach(() => errSpy.mockRestore());

describe("ErrorBoundary", () => {
  it("sin errores renderiza a sus hijos", () => {
    const { getByTestId } = render(
      <ErrorBoundary>
        <Boom explode={false} />
      </ErrorBoundary>,
    );
    expect(getByTestId("ok")).toBeTruthy();
  });

  it("ante un fallo de renderizado no pinta nada (ventana transparente) e informa una vez", () => {
    const onReport = vi.fn();
    const { container } = render(
      <ErrorBoundary onReport={onReport}>
        <Boom explode />
      </ErrorBoundary>,
    );
    expect(container.innerHTML).toBe("");
    expect(onReport).toHaveBeenCalledTimes(1);
    const [kind, msg] = onReport.mock.calls[0] as [string, string];
    expect(kind).toBe("render");
    expect(msg).toContain("falla de prueba");
  });

  it("el informe contiene solo nombre, mensaje y pocas líneas de la pila de componentes", () => {
    const onReport = vi.fn();
    render(
      <ErrorBoundary onReport={onReport}>
        <Boom explode />
      </ErrorBoundary>,
    );
    const msg = (onReport.mock.calls[0] as [string, string])[1];
    expect(msg.length).toBeLessThan(800);
    expect(msg).not.toMatch(/[A-Za-z]:\\/); // sin rutas de disco de la persona
  });

  it("se recupera cuando llega una vista nueva y el hijo ya no falla", () => {
    const { container, rerender, queryByTestId } = render(
      <ErrorBoundary resetKey={1} onReport={() => {}}>
        <Boom explode />
      </ErrorBoundary>,
    );
    expect(container.innerHTML).toBe("");
    rerender(
      <ErrorBoundary resetKey={2} onReport={() => {}}>
        <Boom explode={false} />
      </ErrorBoundary>,
    );
    expect(queryByTestId("ok")).not.toBeNull();
  });

  it("no entra en bucle: tras MAX_RETRIES reintentos fallidos se queda en blanco", () => {
    const onReport = vi.fn();
    const { container, rerender } = render(
      <ErrorBoundary resetKey={0} onReport={onReport}>
        <Boom explode />
      </ErrorBoundary>,
    );
    for (let k = 1; k <= MAX_RETRIES + 5; k++) {
      rerender(
        <ErrorBoundary resetKey={k} onReport={onReport}>
          <Boom explode />
        </ErrorBoundary>,
      );
    }
    expect(container.innerHTML).toBe("");
    expect(onReport).toHaveBeenCalledTimes(MAX_RETRIES + 1); // 1 fallo inicial + 1 por reintento, luego nada
  });

  it("sin cambio de vista no reintenta", () => {
    const onReport = vi.fn();
    const { rerender } = render(
      <ErrorBoundary resetKey={7} onReport={onReport}>
        <Boom explode />
      </ErrorBoundary>,
    );
    rerender(
      <ErrorBoundary resetKey={7} onReport={onReport}>
        <Boom explode />
      </ErrorBoundary>,
    );
    expect(onReport).toHaveBeenCalledTimes(1);
  });
});
