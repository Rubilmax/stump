import type { AxiosInstance } from 'axios'

import type { Api } from '../../api'
import { OPDSLegacyAPI } from '../opds-legacy-api'

function controller(xml: string) {
	const get = vi.fn().mockResolvedValue({ data: xml })
	return new OPDSLegacyAPI({
		axios: { get } as unknown as AxiosInstance,
		serviceURL: 'http://localhost/api/v2',
	} as unknown as Api)
}

test('parses OPDS XML entities, numeric text, singleton entries, and link attributes', async () => {
	const api = controller(`
		<feed xmlns="http://www.w3.org/2005/Atom">
			<id>catalog</id><title>Library &amp; Books</title>
			<author><name>1942</name></author>
			<link rel="self" href="/opds/v1.2/catalog" type="application/atom+xml" />
			<entry>
				<id>1</id><title>1984</title>
				<author><name>1942</name></author>
				<link rel="http://opds-spec.org/acquisition" href="/books/1.epub" type="application/epub+zip" />
			</entry>
		</feed>
	`)

	const feed = await api.catalog()

	expect(feed.title).toBe('Library & Books')
	expect(feed.author?.name).toBe('1942')
	expect(feed.entries).toHaveLength(1)
	expect(feed.entries[0]?.title).toBe('1984')
	expect(feed.entries[0]?.authors[0]?.name).toBe('1942')
	expect(feed.entries[0]?.links[0]).toMatchObject({
		href: '/books/1.epub',
		type: 'application/epub+zip',
	})
})

test('rejects responses without an OPDS feed', async () => {
	await expect(controller('<html>Not a feed</html>').catalog()).rejects.toThrow(
		'Invalid OPDS feed format',
	)
})
