import { constructLegacySearchURL, constructSearchURL, createLatestOnlyQueue } from '../opdsUtils'

describe('constructSearchURL', () => {
	it('should return URL unchanged when no template section exists', () => {
		const url = '/opds/v2.0/search'
		expect(constructSearchURL(url, 'test')).toBe(url)
	})

	it('should replace {?query} with encoded query parameter', () => {
		const url = '/opds/v2.0/s/0/1{?query}'
		expect(constructSearchURL(url, 'search terms')).toBe('/opds/v2.0/s/0/1?query=search%20terms')
	})

	it('should handle query with special characters', () => {
		const url = '/opds/v2.0/search{?query}'
		expect(constructSearchURL(url, 'author & title')).toBe(
			'/opds/v2.0/search?query=author%20%26%20title',
		)
	})

	it('should preserve additional query params after template', () => {
		// See https://discord.com/channels/972593831172272148/1462953801169244312
		const url = '/opds/v2.0/s/0/1{?query}&topGroup=s'
		expect(constructSearchURL(url, 'some search terms')).toBe(
			'/opds/v2.0/s/0/1?query=some%20search%20terms&topGroup=s',
		)
	})

	it('should handle multiple params in template but only use query', () => {
		const url = '/opds/v2.0/s/0/1{?query,author}&topGroup=s'
		expect(constructSearchURL(url, 'search')).toBe('/opds/v2.0/s/0/1?query=search&topGroup=s')
	})

	it('should remove template when query param is not available', () => {
		const url = '/opds/v2.0/search{?author,title}'
		expect(constructSearchURL(url, 'test')).toBe('/opds/v2.0/search')
	})

	it('should handle empty query string', () => {
		const url = '/opds/v2.0/search{?query}'
		expect(constructSearchURL(url, '')).toBe('/opds/v2.0/search?query=')
	})

	it('should handle whitespace in template params', () => {
		const url = '/opds/v2.0/search{?query , author}'
		expect(constructSearchURL(url, 'book')).toBe('/opds/v2.0/search?query=book')
	})

	it('should reject non-templated URLs and return them unchanged', () => {
		const url = '/opds/v2.0/search?query=test'
		expect(constructSearchURL(url, 'new search')).toBe(url)
	})
})

describe('constructLegacySearchURL', () => {
	it('should replace {searchTerms} with encoded query', () => {
		const url = '/opds/v1.2/series?search={searchTerms}'
		expect(constructLegacySearchURL(url, 'Fire')).toBe('/opds/v1.2/series?search=Fire')
	})

	it('should handle query with spaces', () => {
		const url = '/opds/v1.2/series?search={searchTerms}'
		expect(constructLegacySearchURL(url, 'Fire Power')).toBe(
			'/opds/v1.2/series?search=Fire%20Power',
		)
	})

	it('should return URL unchanged when no template exists', () => {
		const url = '/opds/v1.2/series?search=existing'
		expect(constructLegacySearchURL(url, 'new')).toBe(url)
	})

	it('should handle empty query string', () => {
		const url = '/opds/v1.2/series?search={searchTerms}'
		expect(constructLegacySearchURL(url, '')).toBe('/opds/v1.2/series?search=')
	})

	it('should replace multiple placeholders if present', () => {
		const url = '/opds/v1.2/search?q={searchTerms}&author={author}'
		expect(constructLegacySearchURL(url, 'test')).toBe('/opds/v1.2/search?q=test&author=test')
	})
})

describe('createLatestOnlyQueue', () => {
	it('replaces pending values while a send is in flight', async () => {
		let releaseFirst: () => void = () => undefined
		const firstSend = new Promise<void>((resolve) => {
			releaseFirst = resolve
		})
		const send = vi.fn(async (value: number) => {
			if (value === 1) await firstSend
		})
		const queue = createLatestOnlyQueue(send)

		const drained = queue.push(1)
		void queue.push(2)
		void queue.push(3)
		releaseFirst()
		await drained

		expect(send.mock.calls).toEqual([[1], [3]])
	})

	it('can continue after the sender handles a failure', async () => {
		let releaseFirst: () => void = () => undefined
		const firstSend = new Promise<void>((_, reject) => {
			releaseFirst = () => reject(new Error('offline'))
		})
		const send = vi.fn((value: number) => (value === 1 ? firstSend : Promise.resolve()))
		const queue = createLatestOnlyQueue((value: number) => send(value).catch(() => undefined))

		const drained = queue.push(1)
		void queue.push(2)
		releaseFirst()
		await drained

		expect(send.mock.calls).toEqual([[1], [2]])
	})
})
