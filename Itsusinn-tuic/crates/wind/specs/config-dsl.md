Draft: config-dsl-01
Category: Experimental / Specification Draft
Date: September 2026
Language version: 3

# Config DSL: Static Configuration Description

[简体中文](config-dsl.zh_CN.md)

## Status of This Memo

This is a proposed repository-level standard, not an adopted Wind API or an
Internet standard. It extracts the static XML dialect introduced by `tuic-docs`
into a contract for other configuration generators. Wind does not yet implement
this DSL. The draft revision `config-dsl-01` and the language attribute
`version="3"` identify different things.

The requirements describe the proposed contract. Appendix A records differences
from the reference implementation; acceptance by that implementation alone does
not establish conformance. The English and Chinese editions have matching section
numbers and requirements and are maintained together.

## Abstract

Config DSL statically describes inputs, defaults, applicability conditions,
output structure, fixed conversions, and secret redaction. Consumers can derive
forms, validate input, and project structured configuration values. TOML, JSON,
and YAML are downstream encodings. The description contains no executable code.

## 1. Scope and terminology

Uppercase **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** express the
requirement levels of BCP 14 ([RFC 2119](https://www.rfc-editor.org/rfc/rfc2119),
[RFC 8174](https://www.rfc-editor.org/rfc/rfc8174)). Lowercase uses are ordinary
prose. Sections 1–11 and Section 12.2 are normative for this draft; examples and
Appendix A are informative.

| Term | Meaning |
| --- | --- |
| Description | One DSL document, independent of an input session |
| Consumer | A parser, validator, and projector implementing this contract |
| Host | Application providing editing, target-specific validation and export |
| Root | Object containing top-level input fields and collections |
| Row | Current input object; initially the root, replaced during collection mapping or selection |
| Projection | Structured output produced from one immutable input snapshot |
| Missing | Internal absence marker, distinct from every data value |
| Profile | Separately documented target-specific rules and host requirements |

The DSL does not replace the target application's configuration parser or wire
protocol. Import, migration, includes, arbitrary functions, assignment, recursion,
network lookups, and executable expressions are outside version 3. Consumers MUST
NOT evaluate strings as Rust, JavaScript, shell, templates, or another programming
language. Serde, quick-xml, Rust, and Leptos are implementation choices, not language requirements.

## 2. Processing and data model

A consumer MUST perform these logical phases:

1. Parse the entire description and validate its structure.
2. Register symbols; resolve references and reject duplicates and cycles,
   including unused definitions.
3. Initialize input data from literal defaults and independent row templates.
4. Validate applicable inputs and the host profile's cross-field constraints.
5. Project one input snapshot into an output tree.
6. Derive a redacted preview or serialize the original projection for export.

Hosts MAY retain temporarily invalid edits but MUST NOT export until applicable
validation succeeds. Failures MUST NOT produce partially exported configurations.

Values are Unicode strings, booleans, exact integers, ordered lists, and objects
with unique string keys. Floating-point values and data `null` are not in version
3. Integers MUST NOT be rounded through binary64. Objects are compared without
regard to key order; list order is significant.

Consumers MUST distinguish `Missing`, false, zero, empty strings, empty lists, and
empty objects. Missing input is an error, not an implicit default. Defaults
initialize a session; they MUST NOT silently repair incomplete input snapshots.
`Missing` is not a literal, input default, or serializable value.

## 3. Lexical format

### 3.1. Encoding and structural grammar

Documents MUST be UTF-8 without a BOM. Permitted characters are U+0009, U+000A,
U+000D, U+0020–U+D7FF, U+E000–U+FFFD, and U+10000–U+10FFFF, the `Char` ranges of
[XML 1.0](https://www.w3.org/TR/xml/#charsets). This is an XML variant, not a
general-purpose XML processor; other XML processing rules are not imported.

Names are case-sensitive; matching start/end tags and matching attribute quotes
are mandatory. Duplicate attributes, non-whitespace inter-element text, and
trailing content MUST be rejected. Comments are allowed between elements and MUST
NOT contain `--` internally. XML declarations, DTDs, entity declarations,
namespaces, processing instructions, CDATA, and mixed content MUST be rejected.

The following PEG uses `~` for sequence, `|` for ordered choice, `*`/`+` for
repetition, `!` for negative lookahead, and `ANY` for one permitted character.
Tag matching, entity validity, and semantic constraints apply in addition.

```text
Document  <- SOI ~ Gap* ~ Element ~ Gap* ~ EOI
Gap       <- Space | Comment
Space     <- " " | "\t" | "\r" | "\n"
Comment   <- "<!--" ~ (!"--" ~ ANY)* ~ "-->"
Element   <- Empty | Paired
Empty     <- "<" ~ XmlName ~ (Space+ ~ Attribute)* ~ Space* ~ "/>"
Paired    <- "<" ~ XmlName ~ (Space+ ~ Attribute)* ~ Space* ~ ">"
             ~ Gap* ~ (Element ~ Gap*)* ~ "</" ~ XmlName ~ Space* ~ ">"
XmlName   <- [A-Za-z_] ~ [A-Za-z0-9_-]*
Attribute <- XmlName ~ Space* ~ "=" ~ Space* ~ Quoted
Quoted    <- '"' ~ (!('"' | "<") ~ ANY)* ~ '"'
           | "'" ~ (!("'" | "<") ~ ANY)* ~ "'"
```

### 3.2. Attribute decoding and literal syntax

The only named entities are `&amp;`, `&lt;`, `&gt;`, `&quot;`, and `&apos;`.
Decimal `&#DIGITS;` and hexadecimal `&#xHEXDIGITS;` references MUST have at least
one digit and identify a permitted character. Signs, unknown entities, missing
semicolons, surrogates, and out-of-range code points MUST be rejected. Decoding
occurs once: `&amp;lt;` becomes text `&lt;`, not `<`.

Decoded whitespace, including literal tabs and line endings, MUST be preserved.
Consumers MUST NOT implicitly normalize XML attribute whitespace, line endings,
Unicode text, or trim values. A generic XML library may require adaptation.

| Form after entity decoding | Syntax |
| --- | --- |
| Identifier | `[A-Za-z_][A-Za-z0-9_-]*` |
| Boolean | Exactly `true` or `false` |
| Unsigned integer | `[0-9]+`, decimal |
| Signed integer | `-?[0-9]+`, decimal |

Leading zeros are accepted. Plus signs, fractional/exponent notation, and
surrounding whitespace are not. Numeric `-0` equals zero. Labels, ordinary
strings, and enum option values need not be identifiers.

## 4. Document structure and references

The root MUST be `config-dsl` with required `version="3"` and `target-version`
attributes only. `target-version` is opaque metadata, not a language selector.
The root contains exactly one `inputs` and one `outputs`, and at most one
`conditions` and one `values`. These sections have no attributes. Order is
insignificant and empty sections are allowed. Unknown elements or attributes MUST
be rejected even in inactive branches. Forward references are allowed.

`inputs` contains fields and collections sharing one top-level identifier
namespace. Each collection has its own field namespace. `conditions` contains
`condition name="ID"`, each with one condition element. `values` contains
`value name="ID"`, each with one value element. Condition and value names use
separate namespaces. Duplicate names within a namespace MUST be rejected.

Paths are exactly `/ID` (root) or `ID` (current row). There is no dotted path,
parent traversal, wildcard, implicit array index, or fallback to an enclosing row.
Outside collection operations, row equals root. An output object does not change
input scope. Output names are never interpreted as input paths.

`when` and `use ref` refer to conditions; other `ref` attributes refer to named
values. Named conditions inherit the caller's row. Named values always evaluate
with root and row both set to the root, including when called inside a collection.

Consumers MUST reject undeclared symbols and paths not naming any declared input
in a permitted scope. They MUST check actual scope membership when reading a
field and SHOULD reject provably invalid scopes earlier. Cycle detection MUST
include `when`, `use`, and value references across both namespaces, regardless
of current input values or whether the definitions are used.

## 5. Inputs and defaults

### 5.1. Fields

These are the complete attributes of `field`:

| Attribute | Presence and meaning |
| --- | --- |
| `name` | Required input identifier |
| `type` | Required: `string`, `boolean`, `integer`, or `enum` |
| `default` | Required literal initial value, decoded using `type` |
| `label` | Required plain-text UI label |
| `placeholder`, `hint`, `section` | Optional UI strings; default empty |
| `widget` | Optional `text`, `password`, `number`, or `email`; only for `string` |
| `when` | Optional named applicability condition; default applicable |
| `rule` | Optional validation rule identifier; absent/empty means no additional rule |

String defaults preserve text. Booleans follow Section 3.2. Input integers range
from 0 through 18446744073709551615. An enum is a string with one or more child
`option` elements, each having required `value` and `label` attributes and no
children. Option values MUST be unique and the default MUST belong to them.
Non-enum fields MUST have no children.

Widgets do not change data types: `type="string" widget="number"` retains editable
text. Password widgets do not imply output redaction. Metadata MUST be displayed
as text, never executed as markup. Layout, section names, and localization belong
to the host.

False `when` excludes a field from ordinary validation and display but MUST NOT
erase its stored value. Applicability is not access control and does not suppress
an output reading the field; output conditions are separate.

### 5.2. Collections

`collection` has required `name` and `initial-items`, optional `when`, and one or
more uniquely named `field` children. Nested input collections are not allowed.
`initial-items` is an unsigned integer in 0–1000. Initial rows are independent
copies of the template, regardless of applicability. Add-row operations MUST use
that template before any documented host interaction behavior.

Collection `when` evaluates at the root. If false, its rows are excluded from
ordinary validation/display. Field `when` evaluates within each applicable row.
Stable UI IDs, selected-user controls, credential generation, and deletion
behavior are host concerns and are not automatically exported.

### 5.3. Profiles and validation rules

A profile MUST document each supported nonempty `rule`. Unsupported rules MUST
be rejected, not silently skipped or resolved as runtime function names. Version
3 has no XML `profile` attribute; hosts identify supported profiles separately.
A consumer claiming the **TUIC generator profile** MUST support these rules:

| Rule | Constraint on applicable string input |
| --- | --- |
| `required` | Nonempty after Unicode whitespace trimming |
| `password` | Raw length greater than zero; whitespace remains significant |
| `port` | ASCII decimal digits in 1–65535 |
| `milliseconds`, `seconds` | ASCII decimal digits in 1–9007199254740991, in the named unit |
| `socks-credential` | `required`, and at most 255 UTF-8 bytes before trimming |
| `uuid` | Trim/lowercase; canonical 8-4-4-4-12 hyphenated, non-nil UUID |
| `socket` | Trim; numeric IP plus valid port; IPv6 requires brackets |
| `endpoint` | As `socket`, also allowing an ASCII domain instead of an IPv4 address |
| `host` | Trim/remove one enclosing bracket pair; IP other than an unspecified address, or ASCII domain |
| `email` | Nonempty part before first `@`; remainder contains a dot, has no `@`, and does not start/end with a dot; no Unicode whitespace anywhere |

Here an ASCII domain is 1–253 bytes, with dot-separated labels of 1–63 bytes,
alphanumeric ends, and only alphanumerics or hyphens within labels; a string
consisting entirely of digits/dots is not a domain. No trailing dot, IDNA, DNS, or
public-suffix check is implicit. IPv4 uses decimal components without leading
zeros. IPv6 has no zone identifier. Bracketed endpoints require IPv6 inside.
These input checks do not establish reachability, mail deliverability, or TLS trust.

The TUIC profile also defines cross-field rules: unique UUIDs, valid selection,
SNI, conflicting listeners, reconnect bounds, and explicit acceptance of
self-signed certificates. The core MUST NOT infer such constraints from field names.

## 6. Conditions

Condition elements cannot carry `name`, `when`, or transformations:

| Element | Attributes | Children and result |
| --- | --- | --- |
| `all` / `any` | None | One or more conditions; all/any must be true |
| `not` | None | One condition; negate result |
| `use` | Required `ref` | None; named condition in caller's row |
| `eq` | Exactly one of `from`/`ref`; required `value` | None; compare scalar text with the literal |
| `truthy` | Exactly one of `from`/`ref` | None; require and return boolean |
| `ip` | Exactly one of `from`/`ref` | None; require string; test IPv4/IPv6 literal |

`all`/`any` MUST evaluate left to right with short-circuiting. Encountered errors
propagate; they do not become false. `eq` preserves strings, renders booleans as
`true`/`false`, and integers as canonical decimal. String `"01"` differs from
`value="1"`, while integer 1 matches. Objects, lists, or missing values are errors.
`truthy` does not coerce numbers/strings. `ip` does not trim, unbracket, resolve
DNS, or accept CIDR/zone suffixes.

## 7. Value descriptions and conversions

### 7.1. Value elements

Every value element accepts optional `when`, checked before any source or child
evaluation. False produces `Missing`. Other attributes/children are:

| Element | Attributes other than `when` | Children |
| --- | --- | --- |
| `source` | Exactly one of `from`/`ref`; optional `transform` | None |
| `coalesce` | None | One or more value elements |
| `endpoint` | None | Exactly two value elements: host and port |
| `select` | Required `from` and `index`, both data paths | Exactly one value element |

`coalesce` returns the first result other than `Missing` or the empty string.
False, zero, empty lists and objects MUST be retained. No candidate means `""`.
Lookup, conversion and type errors MUST propagate rather than try the next child.

`endpoint` requires a host string and unsigned integer port in 1–65535. It emits
`[host]:port` for a recognized unbracketed IPv6 literal and `host:port` otherwise,
preserving host spelling and using canonical decimal for the port. It performs
no DNS lookup or additional hostname validation; the input profile supplies that.

`select` reads its list and zero-based nonnegative integer index in the caller's
context. It evaluates the child with the selected row and unchanged root. Missing
data, wrong types, or out-of-range indices are errors. Numeric strings are not indices.

### 7.2. Fixed conversions

`transform` and `key-transform`, when present, contain a nonempty ordered sequence
of these operation identifiers, separated by Unicode `White_Space`. An absent
attribute means no conversion. Every operation requires string input:

| Operation | Result |
| --- | --- |
| `trim` | Remove leading/trailing Unicode `White_Space` |
| `lowercase` | Locale-independent Unicode lowercase conversion, not case folding |
| `unbracket` | Remove one leading `[` and trailing `]` pair if both exist; otherwise preserve text |
| `integer` | Parse ASCII decimal digits into an exact integer in 0–18446744073709551615 |

Consumers MUST identify the Unicode data version used. Profiles requiring
interoperable non-ASCII conversions MUST agree on that version; ASCII conversions
are the portable baseline. Implicit normalization, wrapping, saturation, float
parsing, and error-to-zero fallback MUST NOT occur. A string operation after
`integer` fails because the intermediate value is numeric.

Only `string` outputs accept `unit`, whose value MUST be `s` or `ms`. After
conversion it requires an integer and appends that suffix to canonical decimal.
There is no scaling or positivity check; input rules supply those constraints.

## 8. Output projection

### 8.1. Shared rules and scalars

`outputs` has no attributes and constructs the root output object. Its children
and an `object`'s children MUST have unique `name` identifiers. A list/record item
MUST NOT have `name`. All output nodes except `outputs` accept optional `when`
and boolean `secret` (default false).

`when` is evaluated first. If false, the entire node is omitted without reading
its contents. False, zero, and empty strings MUST NOT imply omission. `Missing`
reaching scalar type validation is an error, not implicit omission.

Scalar elements are `string`, `boolean`, `integer`, and `enum`. They accept the
shared attributes and `from`, `ref`, `value`, `transform`, plus the type-specific
attributes below. Exactly one source form MUST exist: `from`, `ref`, literal
`value`, or exactly one child value element. Conflicting source forms are errors.

Literal `value` uses the element type: string/enum text, boolean syntax, or signed
integer syntax. Processing order is source/literal/child, then `transform`, then
`unit` if present, then type validation.

| Type | Required result | Additional attributes |
| --- | --- | --- |
| `string` | String | Optional `unit` |
| `boolean` | Boolean, without string coercion | None |
| `integer` | Integer in -9223372036854775808–9223372036854775807 | None |
| `enum` | String in the referenced enum | Required `options`, naming a top-level enum input |

Input integers above the signed output range MUST fail an `integer` projection,
not wrap or truncate.

### 8.2. Objects and collections

`object` accepts only shared attributes and zero or more named output children.
It does not change input scope. Nested objects form nesting explicitly.

`list` accepts shared attributes and optional `from`, `where-field`, `equals`,
and `omit-empty`. It MUST contain one unnamed output child as its item template.
With `from`, the source MUST be a list. Each source row supplies the current row,
preserving order. Without `from`, evaluate the template once in the caller's row,
producing zero or one item; this is not a multiple-literal-child syntax.

`where-field` and `equals` MUST appear together and require `from`. Before mapping
an item, read the field path in its row and compare using `eq`'s scalar text
semantics. A nonmatching row or child omitted by `when` contributes no item,
never `null`.

`record` accepts shared attributes, required `from` and `key`, and optional
`key-transform` and `omit-empty`. It MUST contain one unnamed output child. For
each source-list row, evaluate the child; when not omitted, read `key` in that
row, transform it, and require a nonempty string. Duplicate keys after conversion
MUST fail the projection, never overwrite. Dynamic keys are ordinary strings,
not necessarily identifiers.

For lists/records, `omit-empty` defaults to false. True omits an empty completed
collection. It is not allowed on objects/scalars. Empty objects and collections
otherwise remain data. Output children MUST be evaluated in declaration order;
this governs short-circuiting and encountered errors, not serialized key order.

## 9. Validation and diagnostics

Before using a description, consumers MUST reject lexical/structural errors,
unsupported versions, unknown tags/attributes, invalid cardinality, duplicate
names, invalid default types or enum defaults, unsupported rules/conversions,
undeclared references, cycles, and conflicting sources. Defaults MAY fail profile
rules such as `required`: an empty form can be a valid description.

Evaluation MUST reject missing fields, wrong types, invalid indices, failed
conversions, incompatible output types, and duplicate normalized record keys.
Hosts MUST additionally apply profile cross-field constraints. Condition errors
MUST NOT silently hide fields or count as successful validation.

Diagnostics MUST identify the phase (description, input, projection, serialization)
and a description location or safe logical path. Diagnostics SHOULD use one-based line/column positions. Wording and machine error codes are not standardized. Limit
errors MAY identify the whole document when a precise location is unavailable.

Diagnostics and logs MUST NOT include submitted values, secret-derived record
keys, or sensitive source excerpts. A schema field name and numeric row index
are sufficient; dumping configuration contents is not.

## 10. Redaction and serialization

Preview redaction operates on a successful projection. Consumers MUST check
values against the declared output structure, reject unknown object fields or
wrong types, and replace every `secret="true"` node with eight U+2022 characters:
`••••••••`. Entire objects/collections can be secret. Record keys remain visible;
version 3 does not annotate secret keys. Hosts relying on redaction MUST NOT place
secret data in such keys.

Redaction MUST NOT reevaluate conditions, reread inputs, mutate the projection,
or be used as an export/validation input. Redacted types may differ and are for
display only. A password widget or a suggestive field name does not substitute
for an explicit output marker.

Export uses the original projection. Each advertised serializer MUST preserve
strings, booleans, exact integers, list order, and nesting on a round trip, or
return an explicit unsupported-value error. Strings and dynamic keys MUST be
escaped, never interpolated as source. Formatting and object-key order may differ.

The TUIC generator profile additionally escapes U+007F, U+0085, U+2028 and U+2029
in JSON strings for its JSON5 reader. Export of a selected top-level object must
be explicit: `server` and `client` are profile names, not core reserved words.

## 11. Versions, limits and security

Version-3 consumers MUST reject other versions and unknown syntax. `target-version`
does not implicitly select validators or enable features. Grammar additions and
semantic changes require a language-version decision before adoption. There is
no namespace-based extension escape hatch.

Descriptions MUST NOT exceed 1,048,576 UTF-8 bytes. Element depth starts at zero
for `config-dsl`; depth above 64 MUST be rejected. Each collection's initial count
is at most 1000. Hosts MUST publish additional limits for input rows, output size,
dependency depth, and processing time where imposed. Limit failures MUST be
explicit, without truncation or partial export. Consumers MUST enforce bounds
before uncontrolled recursion/allocation, not only after a recursive parse.

Conformance reports MUST identify draft revision, supported profiles/rules,
serializers, Unicode data version, and implementation limits. Parser-only
conformance is not complete generator conformance.

Descriptions MUST NOT initiate file access, DNS, HTTP, subprocesses, environment
expansion, or code execution. Output paths remain data for the target application.
Credential generation and export are explicit host operations outside the DSL.
Generation does not verify deployed DNS, files, firewalls, TLS, or connectivity.
This draft defines no IANA registrations, MIME registrations, or wire messages.

## 12. Example and conformance cases

### 12.1. Complete example (informative)

This example uses empty secret data and is not a deployable TUIC configuration.

```xml
<config-dsl version="3" target-version="example">
  <inputs>
    <field name="host" type="string" default=" [2001:db8::1] " label="Host"/>
    <field name="port" type="string" default="0443" label="Port" widget="number"/>
    <field name="auth" type="boolean" default="false" label="Authentication"/>
    <field name="active" type="integer" default="0" label="Selected row"/>
    <collection name="users" initial-items="1">
      <field name="key" type="string" default="demo" label="Key"/>
      <field name="secret" type="string" default="" label="Secret" widget="password"/>
    </collection>
  </inputs>
  <conditions>
    <condition name="auth"><truthy from="/auth"/></condition>
  </conditions>
  <values>
    <value name="host"><source from="/host" transform="trim unbracket"/></value>
  </values>
  <outputs>
    <object name="example">
      <string name="server">
        <endpoint><source ref="host"/><source from="/port" transform="integer"/></endpoint>
      </string>
      <boolean name="enabled" from="/auth"/>
      <integer name="retries" value="0"/>
      <list name="alpn"><string value="h3"/></list>
      <string name="selected">
        <select from="/users" index="/active"><source from="key"/></select>
      </string>
      <record name="users" from="/users" key="key" key-transform="trim lowercase" when="auth">
        <string from="secret" secret="true"/>
      </record>
    </object>
  </outputs>
</config-dsl>
```

Default projection:

```json
{"example":{"server":"[2001:db8::1]:443","enabled":false,"retries":0,"alpn":["h3"],"selected":"demo"}}
```

Changing only `auth` to true changes `enabled` to true and adds original output
`users: {"demo":""}`. The redacted preview becomes:

```json
{"example":{"server":"[2001:db8::1]:443","enabled":true,"retries":0,"alpn":["h3"],"selected":"demo","users":{"demo":"••••••••"}}}
```

### 12.2. Required conformance coverage

A conforming consumer MUST cover at least these cases. Error wording may vary;
success/failure and structural results may not.

| ID | Case | Required result |
| --- | --- | --- |
| C01 | Section 12.1 defaults and edit | Results above |
| C02 | Duplicate attribute/output name, mismatched tag, trailing content | Description error |
| C03 | Unknown syntax in a false branch | Description error |
| C04 | DTD/external entity/unknown entity | Error, no external access |
| C05 | `&amp;lt;`, numeric references, literal whitespace | Single decoding and exact preservation |
| C06 | Signed character reference, surrogate, U+0000, overflow | Error |
| C07 | Direct, indirect, unused, and cross-namespace cycles | Description error |
| C08 | Missing evaluated source versus omitted branch | Error versus no source evaluation |
| C09 | Coalesce false/zero/empty containers/empty string/error | Keep data, skip only missing/empty string, propagate error |
| C10 | Plus sign, fraction/exponent, bad index, signed output overflow | Error, no coercion/truncation |
| C11 | Equal normalized record keys | Error, no overwrite |
| C12 | Filtered/omitted items and both omit-empty modes | Preserve order, explicit omission |
| C13 | Nested secrets, unknown preview key, mutation check | Correct redaction, reject unknown key, preserve original |
| C14 | Root/local paths through mapping, select, conditions, named values | Section 4 scopes |
| C15 | Size/depth/count/resource limits | Explicit bounded failure |
| C16 | Quotes, controls, Unicode, dynamic keys, integer boundaries in serializers | Round-trip preservation or explicit format error |

## 13. References

Normative references are [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) and
[RFC 8174](https://www.rfc-editor.org/rfc/rfc8174) for requirement terminology,
and [XML 1.0 character ranges](https://www.w3.org/TR/xml/#charsets) for Section 3.1.
All other lexical behavior is defined here.

## Appendix A. Reference implementation and draft decisions (informative)

The original baseline is `tuic-docs` commit `775a176`; the local reference has
since been migrated from pest to quick-xml + Serde. Relevant files relative to its
`config-generator/` directory are `src/dsl/xml.rs`, `src/dsl/wire.rs`,
`src/dsl/parser.rs`, `src/dsl.rs`, `src/validation.rs`, and `schema/tuic.xml`.
These identify the local working implementation, not a published revision of the
migration. An event pass checks the XML subset and depth before Serde reads the
tagged nodes, lexical attributes, and ordered children. Semantic validation and
projection remain separate; the parsed description is cached and bound to a
Rust/Leptos host. The v3 attribute-whitespace behavior is preserved and regression
tested. This draft does not move code into Wind or update submodule pointers.

Before claiming full conformance, the reference needs review or hardening here:

| Area | Existing behavior / draft decision |
| --- | --- |
| Identifiers | Some attribute identifiers accept a leading hyphen; the draft requires letter/underscore first. |
| Numeric spelling | Some host integer parsers accept leading `+`; the draft forbids it. Character references already reject signs. |
| Empty conversions | Whitespace-only transforms can act as identity; the draft requires an operation. |
| Filter types | Current filtering renders nonscalars as empty text; the draft requires `eq`'s scalar checks. |
| Row scopes | Some invalid local references are caught only at evaluation; provable early checks are recommended. |
| Resource limits | XML byte size and depth are checked before recursive deserialization; dependency expansion and input-state budgets still need preemptive enforcement. |
| Diagnostics | Text locations/paths exist, but stable phase reporting and safe dynamic-key handling require explicit host support. |
| Unicode version | Inherited from Rust's tables; a conformance report must name the version. |

These are draft decisions, not claims that the reference already rejects every
negative case. Non-ASCII conversion versioning and a shared machine-readable
conformance corpus should be resolved before declaring a stable standard.
