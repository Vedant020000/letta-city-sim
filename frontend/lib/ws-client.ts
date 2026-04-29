import { WorldEventEnvelope } from "@/types/world";

type ConnectOptions = {
  url: string;
  onEvent: (event: WorldEventEnvelope) => void;
  onOpen?: () => void;
  onClose?: () => void;
  onError?: (error: string) => void;
};

export function connectWorldEvents({ url, onEvent, onOpen, onClose, onError }: ConnectOptions) {
  let socket: WebSocket | null = null;
  let disposed = false;
  let reconnectDelayMs = 500;
  let reconnectTimer: number | null = null;

  const clearReconnect = () => {
    if (reconnectTimer !== null) {
      window.clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }
  };

  const scheduleReconnect = () => {
    if (disposed) return;
    clearReconnect();
    reconnectTimer = window.setTimeout(() => {
      connect();
    }, reconnectDelayMs);
    reconnectDelayMs = Math.min(reconnectDelayMs * 2, 5000);
  };

  const connect = () => {
    if (disposed) return;

    try {
      socket = new WebSocket(url);
    } catch (error) {
      onError?.(error instanceof Error ? error.message : "Failed to create websocket");
      scheduleReconnect();
      return;
    }

    socket.onopen = () => {
      reconnectDelayMs = 500;
      onOpen?.();
    };

    socket.onmessage = (message) => {
      try {
        const parsed = JSON.parse(String(message.data || "{}")) as WorldEventEnvelope;
        onEvent(parsed);
      } catch (error) {
        onError?.(error instanceof Error ? error.message : "Failed to parse websocket event");
      }
    };

    socket.onerror = () => {
      onError?.("Websocket connection error");
    };

    socket.onclose = () => {
      onClose?.();
      scheduleReconnect();
    };
  };

  connect();

  return () => {
    disposed = true;
    clearReconnect();
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.close();
    }
  };
}
