To understand how a tool transitions from a fictional text intent by the LLM into actual backend processing, we need to look at how Letta handles custom tool creation and execution. The Letta agent itself does not hold the physics or logic of the city. Instead, it holds a Python function that acts as an HTTP client, forwarding the agent's intent to your Convex backend. [docs.letta](https://docs.letta.com/guides/agents/custom-tools/)

Here is exactly how the actual processing works in three steps.

### Step 1. The Letta Tool Schema (The Proxy)
Inside Letta, you define a custom tool so the LLM knows how to use it. The Python code inside this tool does not calculate pathfinding, inventory, or cooking times. It just formats the arguments chosen by the LLM and sends an HTTP POST request to your backend. [docs.letta](https://docs.letta.com/guides/agents/custom-tools/)

```python
# packages/agents/tools/cook.py
import requests

def cook_food(agent_id: str, target_stove_id: str, recipe_name: str) -> str:
    """Cooks a recipe on a specific stove. Use this when the agent needs to cook."""
    payload = {
        "agentId": agent_id, 
        "stoveId": target_stove_id, 
        "recipe": recipe_name
    }
    # This is the "middle arrow" - forwarding the action to Convex
    response = requests.post("https://your_domain.com/api/tools/cook", json=payload)
    return response.text
```
When you deploy a new YAML role, your Letta daemon registers this exact Python function as a tool using Letta's `client.tools.create()` method. [docs.letta](https://docs.letta.com/guides/agents/custom-tools/)

### Step 2. The Next JS Gateway
Your Next JS server receives this HTTP POST request at `/api/tools/cook`. Its only job is to check if the agent is authorised using their API key, and then forward the payload safely to your Convex HTTP action or mutation.

### Step 3. The Convex Mutation (The Actual Execution)
This is where the fictional intent becomes reality. Convex runs the actual TypeScript business logic. It checks if the agent has the right ingredients, verifies the agent is standing next to the stove, and ultimately updates the Entity Component System (ECS).

```typescript
// convex/tools.ts
import { mutation } from "./_generated/server";
import { v } from "convex/values";

export const processCook = mutation({
  args: { 
    agentId: v.id("entities"), 
    stoveId: v.id("entities"), 
    recipe: v.string() 
  },
  handler: async (ctx, args) => {
    // 1. Verify agent is near the stove
    const agentPos = await getComponent(ctx, args.agentId, "position");
    const stovePos = await getComponent(ctx, args.stoveId, "position");
    if (distance(agentPos, stovePos) > 1) {
      return "Failed: You are too far from the stove.";
    }

    // 2. Consume inventory ingredients
    const inventory = await getComponent(ctx, args.agentId, "inventory");
    if (!hasIngredients(inventory.items, args.recipe)) {
      return "Failed: You lack the ingredients.";
    }
    await consumeIngredients(ctx, args.agentId, args.recipe);

    // 3. Update ECS State to trigger frontend animations
    await ctx.db.insert("components", {
      entityId: args.agentId,
      type: "state",
      data: { currentAction: "cooking", timer: 5000 }
    });

    return "Success: You are now cooking " + args.recipe;
  }
});
```

### The Resulting Loop
When the Convex mutation finishes, it returns the string "Success: You are now cooking pasta" back through the Next JS gateway, which Letta receives and injects into the LLM's memory. Simultaneously, Convex instantly broadcasts the new "cooking" component to your Phaser frontend via websockets. The sprite immediately starts the cooking animation. [docs.letta](https://docs.letta.com/guides/agents/custom-tools/)

This strict separation ensures the LLM cannot cheat or hallucinate game state. It can only request an action, and Convex acts as the absolute authority enforcing the physical rules of your city.
