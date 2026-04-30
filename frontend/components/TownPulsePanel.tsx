import { SimEvent, TownPulse } from "@/types/world";

function formatTime(timestamp: string) {
  return new Date(timestamp).toLocaleString();
}

function plural(count: number, singular: string, pluralValue: string) {
  return count === 1 ? singular : pluralValue;
}

function formatSignalType(type: string) {
  return type.replaceAll(".", " ");
}

function humanizeSignal(event: SimEvent, agentNamesById: Map<string, string>) {
  if (event.type === "board.post.created" && typeof event.metadata.text === "string") {
    return `Notice board: ${event.metadata.text}`;
  }

  let description = event.description;
  for (const [agentId, name] of agentNamesById) {
    description = description
      .replaceAll(`Agent ${agentId}`, name)
      .replaceAll(`agent ${agentId}`, name);
  }
  return description;
}

export function TownPulsePanel({ pulse }: { pulse: TownPulse | null }) {
  if (!pulse) {
    return (
      <section className="pulse-panel">
        <div className="pulse-header">
          <div>
            <span className="eyebrow compact">town pulse</span>
            <h2>What&apos;s happening today?</h2>
          </div>
        </div>
        <p className="muted">Loading town pulse...</p>
      </section>
    );
  }

  const agentsWithIntentions = pulse.active_agents
    .filter((agent) => agent.intention_summary)
    .slice(0, 5);
  const visibleAgents = agentsWithIntentions.length > 0
    ? agentsWithIntentions
    : pulse.active_agents.filter((agent) => agent.current_activity).slice(0, 5);
  const agentNamesById = new Map(pulse.active_agents.map((agent) => [agent.agent_id, agent.name]));

  return (
    <section className="pulse-panel">
      <div className="pulse-header">
        <div>
          <span className="eyebrow compact">town pulse</span>
          <h2>What&apos;s happening today?</h2>
        </div>
        <div className="pulse-time">
          <strong>{pulse.world_time.time_of_day}</strong>
          <span>{formatTime(pulse.world_time.timestamp)}</span>
        </div>
      </div>

      <p className="pulse-headline">{pulse.headline}</p>

      <div className="pulse-grid">
        <div className="pulse-card wide">
          <h3>Highlights</h3>
          <div className="pulse-list">
            {pulse.highlights.map((highlight, index) => (
              <div className="pulse-item" key={`${highlight}-${index}`}>
                {highlight}
              </div>
            ))}
          </div>
        </div>

        <div className="pulse-card">
          <h3>Busy places</h3>
          <div className="pulse-list compact-list">
            {pulse.busy_locations.slice(0, 4).map((location) => (
              <div className="pulse-place" key={location.location_id}>
                <strong>{location.name}</strong>
                <span>
                  {location.agent_count} {plural(location.agent_count, "agent", "agents")} · {location.recent_event_count} {plural(location.recent_event_count, "recent event", "recent events")}
                </span>
              </div>
            ))}
            {pulse.busy_locations.length === 0 ? <span className="muted">No busy locations yet.</span> : null}
          </div>
        </div>

        <div className="pulse-card">
          <h3>Recent signals</h3>
          <div className="pulse-list compact-list">
            {pulse.recent_events.slice(0, 4).map((event) => (
              <div className="pulse-signal" key={event.id}>
                <strong>{formatSignalType(event.type)}</strong>
                <span>{humanizeSignal(event, agentNamesById)}</span>
              </div>
            ))}
            {pulse.recent_events.length === 0 ? <span className="muted">No recent public signals yet.</span> : null}
          </div>
        </div>
      </div>

      {visibleAgents.length > 0 ? (
        <div className="pulse-agents">
          {visibleAgents.map((agent) => (
            <div className="pulse-agent" key={agent.agent_id}>
              <strong>{agent.name}</strong>
              <span>
                {agent.primary_job_name || agent.occupation} · {agent.location_name}
              </span>
              <small>{agent.intention_summary || agent.current_activity}</small>
            </div>
          ))}
        </div>
      ) : null}
    </section>
  );
}
