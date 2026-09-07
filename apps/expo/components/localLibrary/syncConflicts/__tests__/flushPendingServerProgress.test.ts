import { flushPendingServerProgress } from '../flushPendingServerProgress'

describe('flushPendingServerProgress', () => {
	it('retains pending servers when any push fails', async () => {
		const pending = new Set(['server-a', 'server-b'])
		const refresh = vi.fn()

		await expect(
			flushPendingServerProgress(
				pending,
				async () => ({
					'server-a': { failureCount: 0 },
					'server-b': { failureCount: 1 },
				}),
				refresh,
			),
		).rejects.toThrow('Failed to push progress')

		expect(Array.from(pending)).toEqual(['server-a', 'server-b'])
		expect(refresh).not.toHaveBeenCalled()
	})

	it('drains servers added while a successful sync is in flight', async () => {
		const pending = new Set(['server-a'])
		const refresh = vi.fn()
		const sync = vi
			.fn()
			.mockImplementationOnce(async () => {
				pending.add('server-a')
				pending.add('server-b')
				return { 'server-a': { failureCount: 0 } }
			})
			.mockResolvedValue({
				'server-a': { failureCount: 0 },
				'server-b': { failureCount: 0 },
			})

		await flushPendingServerProgress(pending, sync, refresh)

		expect(sync).toHaveBeenNthCalledWith(1, ['server-a'])
		expect(sync).toHaveBeenNthCalledWith(2, ['server-a', 'server-b'])
		expect(pending.size).toBe(0)
		expect(refresh).toHaveBeenCalledOnce()
	})

	it('restores the claimed batch when syncing throws', async () => {
		const pending = new Set(['server-a'])
		const refresh = vi.fn()
		const sync = vi.fn(async () => {
			pending.add('server-b')
			throw new Error('offline')
		})

		await expect(flushPendingServerProgress(pending, sync, refresh)).rejects.toThrow('offline')

		expect(Array.from(pending).sort()).toEqual(['server-a', 'server-b'])
		expect(sync).toHaveBeenCalledOnce()
		expect(refresh).not.toHaveBeenCalled()
	})
})
