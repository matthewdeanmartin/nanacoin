# Micromonetization: optional Amazon Associates links

Status: proposal, not implemented. Reviewed October 3, 2026.

Use a small curated hardware shopping list and selected book links on the
public project website to help pay for NanaCoin development. This should be
an ordinary outbound link feature: visitors choose to visit a retailer, and
the household ledger continues to work independently of it.

## What Clearriva already does

The sibling Clearriva project provides a useful small implementation:

- `../clearriva/api/internal/affiliate/affiliate.go` builds
  `https://www.amazon.com/dp/{ASIN}?tag={associate-tag}` using a URL encoder.
  An empty ASIN produces no link; an empty tag produces a plain product URL.
- `AMAZON_ASSOCIATE_TAG` is configuration, rather than a value scattered through
  templates. Book records carry their ASIN and resulting `affiliate_url`.
- The client marks affiliate links with `nofollow sponsored` and protects
  links opened in a new tab with `noopener`.
- Its spec describes ASIN enrichment as unfinished work. We should borrow the
  URL builder pattern, not assume its catalog enrichment or disclosure work
  is complete, or copy its Go backend into NanaCoin.

NanaCoin does not need Clearriva's ETL, database or API for a few editorially
selected links. A checked-in catalog and a small build-time URL helper are
sufficient. No Amazon credentials belong in the Angular bundle; an Associates
tag is public attribution configuration, not an API secret.

## Where to use it

Start with a public **Build NanaCoin / Shopping list** documentation page,
linked from README and the demo's Help menu. The existing shopping list is now
[`archive/nanacoin_go/SHOPPING_LIST.md`](../archive/nanacoin_go/SHOPPING_LIST.md);
it describes historical TinyGo alternatives. Write a current Rust-oriented
list rather than publishing that historical list as current buying advice.

Candidates include the supported ESP32-S3-N16R8 bank board, the supported S2
configuration, an appropriate USB data cable and optional enclosure. Verify
the specific product's chip, flash, PSRAM and connectors against current
firmware requirements before associating an ASIN. Say what is required and
what is optional; explain that the desktop demo needs no purchase.

Book links should be selected reading recommendations in public docs, with
the title, author and edition stated. Verify that the ASIN identifies that
edition and format; ISBN and Wikidata IDs are not automatically ASINs.
Keep available legal free editions, library options and publisher links beside
Amazon links. Do not rewrite arbitrary user-created market listings, messages,
household wishlists or URLs to insert the maintainer's tag.

## Catalog and rendering

Use one small source catalog, for example `docs/shopping/catalog.json`, with:

| Field | Purpose |
| --- | --- |
| `id`, `kind` | Stable key and `hardware` or `book` category |
| `title`, `description` | Our own description and reason for inclusion |
| `requirements` / `edition` | Board compatibility or the exact book format |
| `marketplace`, `asin` | Verified product identifier on an allowed Amazon host |
| `alternatives` | Manufacturer, publisher, free edition or other retailer links |
| `verified_on` | Date the product and compatibility were last checked |

At build time, read `AMAZON_ASSOCIATE_TAG` and an explicit enable setting for
the public site. Begin with one configured marketplace, `www.amazon.com`;
future marketplaces need their own matching tags and verified product records.
Do not infer a location by sending a visitor's IP to a geolocation service.

Validate ASINs as ten ASCII letters/digits, allowlist the marketplace, and use
a URL API to encode the tag. Do not accept a complete untrusted URL as the
affiliate destination. Empty configuration means ordinary untagged links;
missing ASINs mean show alternatives rather than invent a product match.
Use Amazon's supported linking tools to confirm generated link formats before
publishing. A tagged URL alone does not establish eligibility for commissions.

Display **View on Amazon (affiliate link)** next to alternatives, with
`rel="sponsored nofollow noopener"` when opening a new tab. No redirect service,
automatic navigation, ad widgets, tracking pixels, or client-side Amazon SDK is
needed. No data from accounts, messages, balances or ledger entries goes into
the link. Browsing documentation should not contact Amazon until someone clicks.

Build tagged links only for the configured public project/demo origin. Keep
board and local development builds untagged by default. A self-hosting owner
must explicitly configure their own tag and eligible site if they want to use
this feature. Do not silently distribute the project's attribution across
every private household deployment.

## Disclosure and program prerequisites

Before enabling links, enroll the publisher and list the public project
website in the Associates account. Recheck the current agreement and policies
for that site and marketplace. Amazon specifies this disclosure:

> As an Amazon Associate I earn from qualifying purchases.

Put it near the shopping/reading recommendations and on the public site's
affiliate information page, with a short explanation that purchases may support
development. Mark individual links clearly. The disclosure comes from the
[Associates Operating Agreement](https://affiliate-program.amazon.com/help/operating/agreement).

Use our own text initially. Avoid copied retailer images, reviews, prices or
availability claims; dynamic product content would need a separate design
using Amazon-authorized tools and their content rules. Do not plan income from
the maintainer's own purchases or solicit household self-purchases through
their tag. Review distribution restrictions before extending tagged links to
installed apps or other surfaces. These constraints are covered by the
[current Associates Program Policies](https://affiliate-program.amazon.com/help/operating/policies).

The project still needs an actual Associates tag, confirmed product ASINs and
an eligible public site before implementation can be enabled. Do not reuse
Clearriva's example tag or substitute guessed product identifiers.

## Implementation plan and acceptance checks

1. Write the current hardware list and a short reading list, with non-Amazon
   alternatives and verified compatibility/edition information.
2. Add the catalog, URL helper and build-time public-site configuration; generate
   documentation links from the same records to avoid duplicated tags and URLs.
3. Add link labels and disclosures, then test the enabled and disabled builds.
4. Enable only after the owner has configured their account and verified the
   published links and disclosures on the registered public site.

Test URL encoding, invalid ASINs, unsupported hosts, missing configuration,
marketplace/tag selection, correct editions and absence of accidental tagging
on board builds. A browser check should confirm link attributes, visible
disclosures, keyboard access and no third-party requests before a click.
Do not load affiliate product content as part of tests.

Removal should be one configuration change: clear the tag or disable the
feature, regenerate the site, and retain useful untagged shopping/reading links.
Evaluate aggregate results using Amazon's reports rather than adding household
analytics. Revenue is uncertain; a small project's useful recommendations are
the goal, and a larger affiliate catalog is not justified until this is useful.
