/**
 * Mock snapshot generator for demo/fallback mode.
 *
 * Produces a rich, looping BootstrapSnapshot that advances over time so the
 * UI feels alive even when the backend is down.  Call `createMockSnapshotLoop()`
 * to get a controller, then `start()` to begin auto-ticking.
 */

import {
  Agent,
  AgentIntention,
  BootstrapSnapshot,
  BusyLocation,
  Location,
  PulseAgent,
  SimEvent,
  TownPulse,
  WorldEventEnvelope,
  WorldTime,
} from "@/types/world";

// ── Static data ──────────────────────────────────────────────────────────────

const MOCK_LOCATIONS: Location[] = [
  { id: "loc-town-hall", name: "Town Hall", description: "The centre of civic life", map_x: 3, map_y: 2 },
  { id: "loc-market", name: "Market Square", description: "Bustling stalls and trade", map_x: 5, map_y: 3 },
  { id: "loc-tavern", name: "The Rusty Anchor", description: "A warm tavern by the docks", map_x: 7, map_y: 4 },
  { id: "loc-park", name: "Central Park", description: "Green oasis in the town", map_x: 4, map_y: 5 },
  { id: "loc-homes", name: "Residential Row", description: "Quiet neighbourhood", map_x: 2, map_y: 4 },
  { id: "loc-library", name: "The Library", description: "Knowledge and quiet study", map_x: 6, map_y: 2 },
  { id: "loc-farm", name: "Sunrise Farm", description: "Fields on the outskirts", map_x: 1, map_y: 1 },
  { id: "loc-forest", name: "Forest Edge", description: "Wilderness beyond the fences", map_x: 8, map_y: 1 },
];

const LOCATION_IDS = MOCK_LOCATIONS.map((l) => l.id);

const AGENT_DEFS: Array<{
  id: string;
  name: string;
  occupation: string;
  homeLocation: string;
  jobName: string;
  jobKind: string;
}> = [
  { id: "agt-maya", name: "Maya Chen", occupation: "Mayor", homeLocation: "loc-homes", jobName: "Mayor", jobKind: "civic" },
  { id: "agt-alex", name: "Alex Rivera", occupation: "Merchant", homeLocation: "loc-market", jobName: "Stall Keeper", jobKind: "commercial" },
  { id: "agt-sam", name: "Sam Oakley", occupation: "Farmer", homeLocation: "loc-farm", jobName: "Farm Hand", jobKind: "commercial" },
  { id: "agt-luna", name: "Luna Park", occupation: "Scholar", homeLocation: "loc-library", jobName: "Librarian", jobKind: "civic" },
  { id: "agt-kai", name: "Kai Brewer", occupation: "Bartender", homeLocation: "loc-tavern", jobName: "Barkeep", jobKind: "commercial" },
  { id: "agt-nora", name: "Nora Greenwood", occupation: "Ranger", homeLocation: "loc-forest", jobName: "Forest Warden", jobKind: "wild" },
  { id: "agt-eli", name: "Eli Walker", occupation: "Guard", homeLocation: "loc-town-hall", jobName: "Town Guard", jobKind: "civic" },
  { id: "agt-zoe", name: "Zoe Meadows", occupation: "Healer", homeLocation: "loc-park", jobName: "Herbalist", jobKind: "civic" },
];

// ── Time-of-day cycle ────────────────────────────────────────────────────────

const TIME_SLOTS = [
  { label: "dawn", hour: 6 },
  { label: "morning", hour: 8 },
  { label: "midday", hour: 12 },
  { label: "afternoon", hour: 14 },
  { label: "evening", hour: 18 },
  { label: "night", hour: 21 },
];

// ── Agent behaviour cycles ───────────────────────────────────────────────────

type AgentPhase = {
  location: string;
  state: string;
  activity: string | null;
  intentionSummary: string;
  intentionReason: string;
};

const SCHEDULE: Record<string, AgentPhase[]> = {
  "agt-maya": [
    { location: "loc-homes", state: "idle", activity: "Having breakfast", intentionSummary: "Prepare for the day", intentionReason: "Morning routine" },
    { location: "loc-town-hall", state: "idle", activity: "Reviewing decrees", intentionSummary: "Manage town affairs", intentionReason: "Civic duty" },
    { location: "loc-market", state: "walking", activity: "Visiting merchants", intentionSummary: "Inspect market stalls", intentionReason: "Economic oversight" },
    { location: "loc-town-hall", state: "idle", activity: "Holding council", intentionSummary: "Lead town meeting", intentionReason: "Governance" },
    { location: "loc-tavern", state: "idle", activity: "Socialising", intentionSummary: "Relax after work", intentionReason: "Leisure" },
    { location: "loc-homes", state: "sleeping", activity: null, intentionSummary: "Rest for the night", intentionReason: "Need sleep" },
  ],
  "agt-alex": [
    { location: "loc-homes", state: "idle", activity: "Packing goods", intentionSummary: "Prepare market stall", intentionReason: "Opening shop" },
    { location: "loc-market", state: "idle", activity: "Selling wares", intentionSummary: "Trade at market", intentionReason: "Earn income" },
    { location: "loc-market", state: "idle", activity: "Haggling with customers", intentionSummary: "Close deals", intentionReason: "Profit" },
    { location: "loc-tavern", state: "walking", activity: "Heading to tavern", intentionSummary: "Buy supplies", intentionReason: "Restock" },
    { location: "loc-tavern", state: "idle", activity: "Drinking ale", intentionSummary: "Unwind", intentionReason: "Social time" },
    { location: "loc-homes", state: "sleeping", activity: null, intentionSummary: "Sleep", intentionReason: "Rest" },
  ],
  "agt-sam": [
    { location: "loc-farm", state: "idle", activity: "Feeding chickens", intentionSummary: "Morning chores", intentionReason: "Farm work" },
    { location: "loc-farm", state: "idle", activity: "Ploughing fields", intentionSummary: "Tend crops", intentionReason: "Harvest prep" },
    { location: "loc-market", state: "walking", activity: "Delivering produce", intentionSummary: "Sell vegetables", intentionReason: "Income" },
    { location: "loc-farm", state: "idle", activity: "Repairing fence", intentionSummary: "Maintain farm", intentionReason: "Upkeep" },
    { location: "loc-tavern", state: "idle", activity: "Sharing stories", intentionSummary: "Socialise", intentionReason: "Community" },
    { location: "loc-farm", state: "sleeping", activity: null, intentionSummary: "Sleep", intentionReason: "Rest" },
  ],
  "agt-luna": [
    { location: "loc-homes", state: "idle", activity: "Reading journal", intentionSummary: "Morning study", intentionReason: "Curiosity" },
    { location: "loc-library", state: "idle", activity: "Cataloguing books", intentionSummary: "Organise library", intentionReason: "Duty" },
    { location: "loc-library", state: "idle", activity: "Researching lore", intentionSummary: "Deep study", intentionReason: "Knowledge" },
    { location: "loc-park", state: "walking", activity: "Walking in park", intentionSummary: "Fresh air break", intentionReason: "Wellbeing" },
    { location: "loc-library", state: "idle", activity: "Teaching a class", intentionSummary: "Educate citizens", intentionReason: "Service" },
    { location: "loc-homes", state: "sleeping", activity: null, intentionSummary: "Sleep", intentionReason: "Rest" },
  ],
  "agt-kai": [
    { location: "loc-homes", state: "idle", activity: "Preparing ingredients", intentionSummary: "Prep the bar", intentionReason: "Work" },
    { location: "loc-market", state: "walking", activity: "Buying supplies", intentionSummary: "Stock up on ale", intentionReason: "Inventory" },
    { location: "loc-tavern", state: "idle", activity: "Serving drinks", intentionSummary: "Tend the bar", intentionReason: "Customers" },
    { location: "loc-tavern", state: "idle", activity: "Mixing cocktails", intentionSummary: "Craft special drinks", intentionReason: "Reputation" },
    { location: "loc-tavern", state: "idle", activity: "Listening to gossip", intentionSummary: "Close the bar", intentionReason: "End of shift" },
    { location: "loc-homes", state: "sleeping", activity: null, intentionSummary: "Sleep", intentionReason: "Rest" },
  ],
  "agt-nora": [
    { location: "loc-forest", state: "idle", activity: "Patrolling trails", intentionSummary: "Scout the forest", intentionReason: "Safety" },
    { location: "loc-forest", state: "walking", activity: "Tracking wildlife", intentionSummary: "Monitor animals", intentionReason: "Conservation" },
    { location: "loc-park", state: "walking", activity: "Foraging herbs", intentionSummary: "Gather medicinal plants", intentionReason: "Healing supplies" },
    { location: "loc-town-hall", state: "idle", activity: "Reporting findings", intentionSummary: "Update the mayor", intentionReason: "Civic duty" },
    { location: "loc-tavern", state: "idle", activity: "Resting by the fire", intentionSummary: "Evening rest", intentionReason: "Recovery" },
    { location: "loc-forest", state: "sleeping", activity: null, intentionSummary: "Sleep under the stars", intentionReason: "Rest" },
  ],
  "agt-eli": [
    { location: "loc-town-hall", state: "idle", activity: "Morning patrol", intentionSummary: "Guard the town", intentionReason: "Duty" },
    { location: "loc-market", state: "walking", activity: "Patrolling market", intentionSummary: "Keep the peace", intentionReason: "Security" },
    { location: "loc-town-hall", state: "idle", activity: "Guarding entrance", intentionSummary: "Watch for trouble", intentionReason: "Vigilance" },
    { location: "loc-park", state: "walking", activity: "Rounding the park", intentionSummary: "Patrol perimeter", intentionReason: "Safety" },
    { location: "loc-tavern", state: "idle", activity: "Off-duty drink", intentionSummary: "Relax", intentionReason: "Leisure" },
    { location: "loc-homes", state: "sleeping", activity: null, intentionSummary: "Sleep", intentionReason: "Rest" },
  ],
  "agt-zoe": [
    { location: "loc-homes", state: "idle", activity: "Brewing tea", intentionSummary: "Morning meditation", intentionReason: "Wellness" },
    { location: "loc-park", state: "idle", activity: "Tending herb garden", intentionSummary: "Grow remedies", intentionReason: "Medicine" },
    { location: "loc-park", state: "idle", activity: "Treating a patient", intentionSummary: "Heal the sick", intentionReason: "Compassion" },
    { location: "loc-market", state: "walking", activity: "Buying herbs", intentionSummary: "Restock supplies", intentionReason: "Need ingredients" },
    { location: "loc-library", state: "idle", activity: "Studying remedies", intentionSummary: "Research cures", intentionReason: "Knowledge" },
    { location: "loc-homes", state: "sleeping", activity: null, intentionSummary: "Sleep", intentionReason: "Rest" },
  ],
};

// ── Headlines & highlights that cycle ────────────────────────────────────────

const HEADLINES = [
  "The town buzzes with activity as citizens go about their day.",
  "A warm breeze sweeps through Market Square — trade is brisk today.",
  "The library reports a surge in visitors seeking ancient lore.",
  "Ranger Nora spotted deer near the Forest Edge — a good omen.",
  "Mayor Maya announces new civic improvements for the quarter.",
  "The tavern is lively tonight — Kai's special brew is a hit.",
  "Farm yields are up this season thanks to Sam's dedication.",
  "Guard Eli reports a quiet day — no incidents to note.",
];

const HIGHLIGHTS_POOL = [
  "Market trade volume is above average this week.",
  "The herb garden in Central Park is in full bloom.",
  "Town council approved new road repairs.",
  "A travelling merchant arrived with rare goods.",
  "The library acquired a collection of ancient maps.",
  "Wildlife sightings are up near Forest Edge.",
  "Community dinner at the tavern was a success.",
  "New flower beds planted along Residential Row.",
  "The farm's tomato harvest is the best in years.",
  "Scholar Luna discovered a forgotten manuscript.",
];

const EVENT_TEMPLATES: Array<{
  type: string;
  description: (agentName: string, locationName: string) => string;
}> = [
  { type: "location.enter", description: (a, l) => `${a} arrived at ${l}` },
  { type: "location.leave", description: (a, l) => `${a} left ${l}` },
  { type: "agent.work", description: (a, l) => `${a} is working at ${l}` },
  { type: "agent.socialize", description: (a, l) => `${a} is chatting at ${l}` },
  { type: "agent.rest", description: (a, _l) => `${a} is resting` },
  { type: "board.post.created", description: (a, _l) => `${a} posted a notice on the board` },
  { type: "agent.eat", description: (a, l) => `${a} is eating at ${l}` },
  { type: "agent.trade", description: (a, l) => `${a} made a trade at ${l}` },
];

// ── Mock snapshot loop ───────────────────────────────────────────────────────

export type MockSnapshotLoop = {
  getSnapshot: () => BootstrapSnapshot;
  tick: () => void;
  start: (intervalMs?: number) => void;
  stop: () => void;
  isRunning: () => boolean;
};

export function createMockSnapshotLoop(): MockSnapshotLoop {
  let tickCount = 0;
  let timerId: ReturnType<typeof setInterval> | null = null;

  // ── helpers ──

  function currentSlotIndex(): number {
    return tickCount % TIME_SLOTS.length;
  }

  function currentSlot() {
    return TIME_SLOTS[currentSlotIndex()];
  }

  function phaseForAgent(agentId: string): AgentPhase {
    const phases = SCHEDULE[agentId] ?? SCHEDULE["agt-maya"];
    return phases[currentSlotIndex()];
  }

  function locationNameById(id: string): string {
    return MOCK_LOCATIONS.find((l) => l.id === id)?.name ?? id;
  }

  // ── build snapshot ──

  function buildWorldTime(): WorldTime {
    const slot = currentSlot();
    const baseDate = new Date("2025-06-15T00:00:00Z");
    baseDate.setHours(slot.hour, tickCount * 2, 0);
    return {
      timestamp: baseDate.toISOString(),
      time_of_day: slot.label,
      simulation_paused: false,
    };
  }

  function buildAgents(): Agent[] {
    return AGENT_DEFS.map((def) => {
      const phase = phaseForAgent(def.id);
      const slotIdx = currentSlotIndex();
      // Vitals drift with time of day
      const isSleeping = phase.state === "sleeping";
      const isWalking = phase.state === "walking";
      return {
        id: def.id,
        name: def.name,
        occupation: def.occupation,
        current_location_id: phase.location,
        state: phase.state,
        current_activity: phase.activity,
        is_npc: false,
        is_active: !isSleeping,
        state_updated_at: new Date().toISOString(),
        balance_cents: 5000 + (tickCount * 17) % 3000,
        last_income_cents: isWalking ? null : 250 + (slotIdx * 50),
        last_income_reason: isWalking ? null : "wages",
        last_income_at: isWalking ? null : new Date().toISOString(),
        last_expense_cents: isWalking ? null : 80 + (slotIdx * 20),
        last_expense_reason: isWalking ? null : "food",
        last_expense_at: isWalking ? null : new Date().toISOString(),
        food_level: isSleeping ? 45 : 55 + (slotIdx * 6) % 40,
        water_level: isSleeping ? 50 : 60 + (slotIdx * 7) % 35,
        stamina_level: isSleeping ? 90 : isWalking ? 40 : 60 + (slotIdx * 5) % 30,
        sleep_level: isSleeping ? 95 : 80 - slotIdx * 10,
        last_vitals_update: new Date().toISOString(),
      };
    });
  }

  function buildIntentions(): AgentIntention[] {
    return AGENT_DEFS.map((def) => {
      const phase = phaseForAgent(def.id);
      return {
        id: `int-${def.id}-${tickCount}`,
        agent_id: def.id,
        summary: phase.intentionSummary,
        reason: phase.intentionReason,
        status: "active" as const,
        expected_location_id: phase.location,
        expected_action: phase.activity,
        outcome: null,
        metadata: {},
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
        completed_at: null,
      };
    });
  }

  function buildPulseAgents(): PulseAgent[] {
    return AGENT_DEFS.map((def) => {
      const phase = phaseForAgent(def.id);
      return {
        agent_id: def.id,
        name: def.name,
        occupation: def.occupation,
        current_location_id: phase.location,
        location_name: locationNameById(phase.location),
        state: phase.state,
        current_activity: phase.activity,
        intention_summary: phase.intentionSummary,
        intention_reason: phase.intentionReason,
        expected_location_id: phase.location,
        primary_job_id: `job-${def.id}`,
        primary_job_name: def.jobName,
        primary_job_kind: def.jobKind,
      };
    });
  }

  function buildBusyLocations(): BusyLocation[] {
    const agentCounts = new Map<string, number>();
    for (const def of AGENT_DEFS) {
      const phase = phaseForAgent(def.id);
      agentCounts.set(phase.location, (agentCounts.get(phase.location) ?? 0) + 1);
    }
    return Array.from(agentCounts.entries())
      .map(([locId, count]) => ({
        location_id: locId,
        name: locationNameById(locId),
        agent_count: count,
        recent_event_count: 1 + (tickCount % 4),
      }))
      .sort((a, b) => b.agent_count - a.agent_count);
  }

  function buildRecentSimEvents(): SimEvent[] {
    // Generate 6 events that shift with each tick
    const events: SimEvent[] = [];
    for (let i = 0; i < 6; i++) {
      const agentDef = AGENT_DEFS[(tickCount + i) % AGENT_DEFS.length];
      const phase = phaseForAgent(agentDef.id);
      const template = EVENT_TEMPLATES[(tickCount + i) % EVENT_TEMPLATES.length];
      const locName = locationNameById(phase.location);
      events.push({
        id: tickCount * 10 + i,
        occurred_at: new Date(Date.now() - i * 120000).toISOString(),
        type: template.type,
        actor_id: agentDef.id,
        location_id: phase.location,
        description: template.description(agentDef.name, locName),
        metadata: template.type === "board.post.created"
          ? { text: "Looking for rare herbs — will trade fairly. —Zoe" }
          : {},
      });
    }
    return events;
  }

  function buildTownPulse(): TownPulse {
    const headlineIdx = tickCount % HEADLINES.length;
    const highlightStart = (tickCount * 2) % HIGHLIGHTS_POOL.length;
    const highlights = [
      HIGHLIGHTS_POOL[highlightStart % HIGHLIGHTS_POOL.length],
      HIGHLIGHTS_POOL[(highlightStart + 1) % HIGHLIGHTS_POOL.length],
      HIGHLIGHTS_POOL[(highlightStart + 2) % HIGHLIGHTS_POOL.length],
    ];
    return {
      world_time: buildWorldTime(),
      headline: HEADLINES[headlineIdx],
      highlights,
      active_agents: buildPulseAgents(),
      board_posts: [
        { id: "bp-1", text: "Community potluck this weekend at the tavern!", created_at: new Date().toISOString() },
        { id: "bp-2", text: "Lost: one brown goat. Last seen near Forest Edge.", created_at: new Date().toISOString() },
      ],
      recent_events: buildRecentSimEvents(),
      busy_locations: buildBusyLocations(),
    };
  }

  function buildRecentEvents(): WorldEventEnvelope[] {
    return buildRecentSimEvents().map((e) => ({
      id: `mock-event:${e.id}`,
      ts: e.occurred_at,
      type: e.type,
      agent_targets: e.actor_id ? [e.actor_id] : [],
      location_id: e.location_id,
      payload: {
        actor_id: e.actor_id,
        description: e.description,
        metadata: e.metadata,
        source: "mock",
      },
    }));
  }

  function getSnapshot(): BootstrapSnapshot {
    return {
      agents: buildAgents(),
      currentIntentions: buildIntentions(),
      locations: MOCK_LOCATIONS,
      worldTime: buildWorldTime(),
      townPulse: buildTownPulse(),
      recentEvents: buildRecentEvents(),
    };
  }

  function tick() {
    tickCount += 1;
  }

  function start(intervalMs = 8000) {
    if (timerId !== null) return;
    timerId = setInterval(tick, intervalMs);
  }

  function stop() {
    if (timerId !== null) {
      clearInterval(timerId);
      timerId = null;
    }
  }

  function isRunning() {
    return timerId !== null;
  }

  return { getSnapshot, tick, start, stop, isRunning };
}
