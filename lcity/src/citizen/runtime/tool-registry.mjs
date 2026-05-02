import { jsonResult } from "@letta-ai/letta-code-sdk";
import { invokeWorldTool } from "./world-api.mjs";

function isPlainObject(value) {
  return value != null && typeof value === "object" && !Array.isArray(value);
}

function buildToolMessage(result) {
  if (result.ok) {
    if (result.notification?.message) return result.notification.message;
    return "ok";
  }

  return result.data?.error?.message
    || result.data?.message
    || `HTTP ${result.status_code}`;
}

export function buildWorldTools({ config, manifest, emit, wakeEventId }) {
  return (manifest.tools || []).map((toolDefinition) => ({
    label: toolDefinition.name,
    name: toolDefinition.name,
    description: toolDefinition.description,
    parameters: toolDefinition.parameters,
    async execute(toolCallId, args) {
      const input = isPlainObject(args)
        ? args
        : {
            __invalid_input__: true,
          };

      emit("tool_call_started", {
        eventId: wakeEventId,
        name: toolDefinition.name,
        toolCallId,
      });

      let result;
      if (input.__invalid_input__) {
        result = {
          ok: false,
          status_code: 0,
          data: {
            error: {
              code: "invalid_tool_args",
              message: "Expected tool arguments to be a JSON object.",
            },
          },
        };
      } else {
        try {
          result = await invokeWorldTool(config, toolDefinition, input);
        } catch (error) {
          result = {
            ok: false,
            status_code: 0,
            data: {
              error: {
                code: "tool_execution_error",
                message: error instanceof Error ? error.message : String(error),
              },
            },
          };
        }
      }

      emit("tool_call_finished", {
        eventId: wakeEventId,
        name: toolDefinition.name,
        ok: result.ok,
        message: buildToolMessage(result),
      });

      return jsonResult(result);
    },
  }));
}
