import {
	CAPTURE_MARKER,
	CAPTURE_CUSTOM_TYPE,
	CONTEXT_CUSTOM_TYPE,
	captureInstruction,
	contextInvocation,
	retrieveContext,
} from "@stormlightlabs/codex-plugin-stormbuffer";

export function createLifecycle(pi, options = {}) {
	let captureTurn = false;
	let captureScheduled = false;

	return {
		async beforeAgentStart(event, ctx) {
			const prompt = event?.prompt;
			captureTurn = typeof prompt === "string" && prompt.includes(CAPTURE_MARKER);
			captureScheduled = false;
			if (captureTurn) return undefined;

			const invocation = contextInvocation(prompt, options);
			if (!invocation) return undefined;
			try {
				const recall = options.retrieveContext || retrieveContext;
				const context = await recall(invocation, { cwd: ctx?.cwd, timeout: 5_000 });
				return context
					? { message: { customType: CONTEXT_CUSTOM_TYPE, content: context, display: false } }
					: undefined;
			} catch {
				return undefined;
			}
		},

		agentSettled() {
			if (captureTurn || captureScheduled) return;
			captureScheduled = true;
			pi.sendMessage(
				{ customType: CAPTURE_CUSTOM_TYPE, content: captureInstruction(), display: false },
				{ triggerTurn: true, deliverAs: "followUp" },
			);
		},
	};
}

export default function stormbufferExtension(pi) {
	const lifecycle = createLifecycle(pi);
	pi.on("before_agent_start", lifecycle.beforeAgentStart);
	pi.on("agent_settled", lifecycle.agentSettled);
}
