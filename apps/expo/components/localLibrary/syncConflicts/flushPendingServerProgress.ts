type PushResults = Record<string, { failureCount: number } | undefined>

export async function flushPendingServerProgress(
	pendingServerIds: Set<string>,
	sync: (serverIds: string[]) => Promise<PushResults>,
	onSuccess: () => void,
) {
	if (!pendingServerIds.size) return

	while (pendingServerIds.size) {
		const serverIds = Array.from(pendingServerIds)
		serverIds.forEach((serverId) => pendingServerIds.delete(serverId))

		try {
			const pushResults = await sync(serverIds)
			if (serverIds.some((serverId) => (pushResults[serverId]?.failureCount ?? 1) > 0)) {
				throw new Error('Failed to push progress')
			}
		} catch (error) {
			serverIds.forEach((serverId) => pendingServerIds.add(serverId))
			throw error
		}
	}

	onSuccess()
}
