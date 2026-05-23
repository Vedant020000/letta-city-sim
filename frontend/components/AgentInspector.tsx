"use client";

import { Agent } from "@/types/world";

type Props = {
  agent: Agent;
  onClose: () => void;
};

function VitalsBar({ label, value, color }: { label: string; value: number; color: string }) {
  const pct = Math.max(0, Math.min(100, value));
  return (
    <div className="vitals-row">
      <span className="vitals-label">{label}</span>
      <div className="vitals-bar-track">
        <div className="vitals-bar-fill" style={{ width: `${pct}%`, background: color }} />
      </div>
      <span className="vitals-value">{Math.round(pct)}</span>
    </div>
  );
}

function formatCents(cents: number) {
  const dollars = cents / 100;
  return `$${dollars.toFixed(2)}`;
}

function stateLabel(state: string) {
  switch (state) {
    case "sleeping": return "💤 Sleeping";
    case "idle": return "Idle";
    case "walking": return "🚶 Walking";
    default: return state;
  }
}

export function AgentInspector({ agent, onClose }: Props) {
  return (
    <div className="inspector-panel">
      <div className="inspector-header">
        <div>
          <strong className="inspector-name">{agent.name}</strong>
          <span className="inspector-occupation">{agent.occupation}</span>
        </div>
        <button className="inspector-close" onClick={onClose}>✕</button>
      </div>

      <div className="inspector-state">
        <span className={`state-badge ${agent.state}`}>{stateLabel(agent.state)}</span>
        {agent.current_activity && (
          <span className="inspector-activity">{agent.current_activity}</span>
        )}
      </div>

      <div className="inspector-section">
        <h4>Vitals</h4>
        <VitalsBar label="Food" value={agent.food_level} color="#f97316" />
        <VitalsBar label="Water" value={agent.water_level} color="#3b82f6" />
        <VitalsBar label="Stamina" value={agent.stamina_level} color="#22c55e" />
        <VitalsBar label="Sleep" value={agent.sleep_level} color="#a855f7" />
      </div>

      <div className="inspector-section">
        <h4>Finances</h4>
        <div className="inspector-finances">
          <div className="finance-row">
            <span>Balance</span>
            <strong>{formatCents(agent.balance_cents)}</strong>
          </div>
          {agent.last_income_cents != null && agent.last_income_cents > 0 && (
            <div className="finance-row income">
              <span>Last income</span>
              <span>+{formatCents(agent.last_income_cents)} {agent.last_income_reason || ""}</span>
            </div>
          )}
          {agent.last_expense_cents != null && agent.last_expense_cents > 0 && (
            <div className="finance-row expense">
              <span>Last expense</span>
              <span>-{formatCents(agent.last_expense_cents)} {agent.last_expense_reason || ""}</span>
            </div>
          )}
        </div>
      </div>

      <div className="inspector-section">
        <h4>Location</h4>
        <span className="inspector-location">{agent.current_location_id}</span>
      </div>
    </div>
  );
}
