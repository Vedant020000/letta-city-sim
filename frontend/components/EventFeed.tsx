"use client";

import { WorldEventEnvelope } from "@/types/world";

type Props = {
  events: WorldEventEnvelope[];
};

function formatPayload(payload: Record<string, unknown>) {
  try {
    const serialized = JSON.stringify(payload, null, 2);
    return serialized.length > 360 ? `${serialized.slice(0, 360)}...` : serialized;
  } catch {
    return "<unserializable payload>";
  }
}

function resolveEventType(event: WorldEventEnvelope) {
  return event.type || event.event_type || "unknown";
}

export function EventFeed({ events }: Props) {
  if (events.length === 0) {
    return <div className="muted">No websocket events received yet.</div>;
  }

  return (
    <div className="feed-list">
      {events.map((event) => (
        <article className="feed-card" key={event.id}>
          <div className="feed-card-header">
            <strong>{resolveEventType(event)}</strong>
            <span>{new Date(event.ts).toLocaleTimeString()}</span>
          </div>
          <div className="feed-meta">
            location: {event.location_id || "-"} · targets: {event.agent_targets.length}
          </div>
          <pre className="feed-payload">{formatPayload(event.payload)}</pre>
        </article>
      ))}
    </div>
  );
}
