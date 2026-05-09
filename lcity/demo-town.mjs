import { startTownMap } from "./src/town/ui/map.mjs";

const mockLocations = [
  { id: "loc_cafe", name: "Cafe", description: "Coffee shop", map_x: 0, map_y: 0 },
  { id: "loc_park", name: "Park", description: "Central park", map_x: 3, map_y: 1 },
  { id: "loc_shop", name: "Shop", description: "General store", map_x: 1, map_y: 3 },
  { id: "loc_home", name: "Home", description: "Residential", map_x: 4, map_y: 4 },
  { id: "loc_office", name: "Office", description: "Workplace", map_x: 5, map_y: 0 },
  { id: "loc_diner", name: "Diner", description: "Food spot", map_x: 2, map_y: 5 },
];

const mockAgents = [
  { id: "agent_alice", name: "Alice", occupation: "Barista", current_location_id: "loc_cafe", state: "active", current_activity: "Brewing coffee", is_npc: true, is_active: true, state_updated_at: new Date().toISOString(), balance_cents: 1250, last_income_cents: 500, last_income_reason: "Wage", last_income_at: new Date().toISOString(), last_expense_cents: null, last_expense_reason: null, last_expense_at: null, food_level: 85, water_level: 70, stamina_level: 90, sleep_level: 80, last_vitals_update: new Date().toISOString() },
  { id: "agent_bob", name: "Bob", occupation: "Gardener", current_location_id: "loc_park", state: "active", current_activity: "Watering plants", is_npc: true, is_active: true, state_updated_at: new Date().toISOString(), balance_cents: 890, last_income_cents: 300, last_income_reason: "Wage", last_income_at: new Date().toISOString(), last_expense_cents: null, last_expense_reason: null, last_expense_at: null, food_level: 60, water_level: 55, stamina_level: 75, sleep_level: 65, last_vitals_update: new Date().toISOString() },
  { id: "agent_carol", name: "Carol", occupation: "Clerk", current_location_id: "loc_shop", state: "idle", current_activity: "Stocking shelves", is_npc: true, is_active: true, state_updated_at: new Date().toISOString(), balance_cents: 2100, last_income_cents: 450, last_income_reason: "Wage", last_income_at: new Date().toISOString(), last_expense_cents: 150, last_expense_reason: "Lunch", last_expense_at: new Date().toISOString(), food_level: 92, water_level: 88, stamina_level: 85, sleep_level: 90, last_vitals_update: new Date().toISOString() },
  { id: "agent_dave", name: "Dave", occupation: "Developer", current_location_id: "loc_office", state: "active", current_activity: "Writing code", is_npc: true, is_active: true, state_updated_at: new Date().toISOString(), balance_cents: 3400, last_income_cents: 800, last_income_reason: "Salary", last_income_at: new Date().toISOString(), last_expense_cents: null, last_expense_reason: null, last_expense_at: null, food_level: 78, water_level: 82, stamina_level: 70, sleep_level: 60, last_vitals_update: new Date().toISOString() },
  { id: "agent_eve", name: "Eve", occupation: "Chef", current_location_id: "loc_diner", state: "active", current_activity: "Cooking lunch", is_npc: true, is_active: true, state_updated_at: new Date().toISOString(), balance_cents: 1560, last_income_cents: 600, last_income_reason: "Tips", last_income_at: new Date().toISOString(), last_expense_cents: null, last_expense_reason: null, last_expense_at: null, food_level: 95, water_level: 90, stamina_level: 88, sleep_level: 92, last_vitals_update: new Date().toISOString() },
];

const mockWorldTime = { timestamp: new Date().toISOString(), time_of_day: "morning", simulation_paused: false };
const mockEvents = [
  { id: 1, occurred_at: new Date(Date.now() - 300000).toISOString(), type: "agent_moved", actor_id: "agent_alice", location_id: "loc_cafe", description: "Alice moved to Cafe", metadata: {} },
  { id: 2, occurred_at: new Date(Date.now() - 240000).toISOString(), type: "agent_activity", actor_id: "agent_bob", location_id: "loc_park", description: "Bob started watering plants", metadata: {} },
  { id: 3, occurred_at: new Date(Date.now() - 180000).toISOString(), type: "conversation", actor_id: "agent_alice", location_id: "loc_cafe", description: "Alice says: Morning Bob!", metadata: {} },
  { id: 4, occurred_at: new Date(Date.now() - 120000).toISOString(), type: "agent_activity", actor_id: "agent_carol", location_id: "loc_shop", description: "Carol stocked new inventory", metadata: {} },
  { id: 5, occurred_at: new Date(Date.now() - 60000).toISOString(), type: "economy.credit", actor_id: "agent_dave", location_id: "loc_office", description: "Dave received salary", metadata: {} },
  { id: 6, occurred_at: new Date(Date.now() - 30000).toISOString(), type: "agent_activity", actor_id: "agent_eve", location_id: "loc_diner", description: "Eve started cooking lunch", metadata: {} },
];

global.fetch = async (url) => {
  if (url.includes("/locations")) {
    return { json: async () => ({ data: mockLocations }) };
  }
  if (url.includes("/agents")) {
    return { json: async () => ({ data: mockAgents }) };
  }
  if (url.includes("/world/time")) {
    return { json: async () => ({ data: mockWorldTime }) };
  }
  if (url.includes("/events/recent")) {
    return { json: async () => ({ data: mockEvents }) };
  }
  return { json: async () => ({ data: [] }) };
};

await startTownMap({ apiBase: "http://mock", simKey: "mock", pollMs: 5000 });
