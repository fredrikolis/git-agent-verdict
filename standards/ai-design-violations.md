<!-- Concern: names the visual defaults that mark a UI, slide, diagram, chart or layout as unexamined AI output | Non-concern: the wording of any text on it, and the code behind it | IO: none -->
# AI Design Violations

Every item is a default the model reaches for absent a decision. One or two is a choice; most of them together is nobody choosing. Review cites each one present by its id and location; the fix is a decision, not a different default.

## Color violations

C1. purple/indigo-to-blue diagonal gradient
C2. neon cyan/violet on near-black with glowing borders
C3. dark mode by reflex, not by use
C4. a dark theme made by inverting the light one
C5. radial glow blobs or aurora behind the hero
C6. gradient fill on the headline text
C7. a new hue per section or per item, none carrying meaning
C8. every hue at full saturation
C9. palette is the framework's default (slate ground, grey-500 text, blue-600 button), matches no brand
C10. mid-grey body text on dark below AA contrast
C11. pure white page because no ground color was decided
C12. green/amber/red used for things that are not states

## Type violations

T1. Inter, Roboto, Geist, Space Grotesk or the system stack as the unexamined default
T2. one italic-serif accent word inside a sans headline
T3. tracked-out ALL-CAPS eyebrow over every heading
T4. oversized headline over cramped body
T5. every heading the same size
T6. two type sizes doing the work of a hierarchy
T7. monospace on small data labels as a look
T8. Title Case on every label
T9. centered body paragraphs
T10. lines past ninety characters
T11. justified text
T12. uneven tracking or baselines across repeated elements

## Layout violations

L1. centered hero, pill badge above the H1, two side-by-side buttons
L2. three or six identical cards in a row, icon over heading over two lines
L3. a banner row of big numbers
L4. 01/02/03 markers on content that is not a sequence
L5. every section the same padding, width and rhythm
L6. everything centered
L7. content chopped into equal grid cells regardless of weight
L8. nothing dominant, everything equally important
L9. no column grid, elements aligned to nothing
L10. landing-page spacing applied to a tool people use all day
L11. the layout could be swapped with any other product's

## Announcing violations

A1. an eyebrow, a title and a subtitle stacked above content that is one line: collapse to that line
A2. a heading that names the category of what follows ("The core of the issue") instead of stating it
A3. a subheading that restates the heading in more words
A4. three tiers of type where one tier carries the content and the other two point at it
A5. a slide titled with its topic while the point sits in the body: the point is the title
A6. a section label ("Overview", "Introduction", "Key takeaways") over content that already is that
A7. a card title repeating the section heading above it
A8. a lead-in line above a chart saying what it is about, when the chart title should say what it shows
A9. an intro section or slide listing what is coming instead of starting
A10. a heading and a bold first sentence saying the same thing

## Page structure violations

P1. hero → features → how it works → testimonials → pricing → CTA → footer, in that order, for a product that needed two of them
P2. logo left, four links, one button right
P3. three pricing tiers with the middle one highlighted
P4. a logo wall of invented logos
P5. five-star testimonial cards
P6. an FAQ accordion
P7. a four-column footer of links to nothing
P8. a hamburger hiding three links
P9. a section for every noun in the brief

## Container violations

B1. a card for every block, cards nested in cards
B2. a colored left or top border on the card
B3. one border-radius on everything regardless of hierarchy
B4. the same soft grey shadow under everything
B5. frosted blur plus a 1px light border over a gradient
B6. a 1px border on everything
B7. padding generous and uniform where the content is small
B8. a border, a shadow and a background tint all on the same box

## Control violations

K1. a gradient or glowing primary button
K2. a primary button in every section, several per view
K3. fully rounded pill buttons throughout
K4. a toggle switch for every boolean
K5. placeholder text standing in for the label
K6. icon-only buttons with no label
K7. every input the same width regardless of what goes in it
K8. no visible focus state
K9. disabled and enabled states told apart by opacity alone

## Decoration violations

D1. gradient washes as decoration
D2. glows and colored box-shadows
D3. colored status dots that carry no state
D4. a rule under every heading
D5. pills and badges on everything
D6. blob, wave or brushstroke background shapes
D7. a divider between every pair of items
D8. a dotted or grid background texture behind the hero
D9. decorative sparkles or stars on anything labeled AI

## Icon and imagery violations

I1. emoji as icons in nav, bullets or headings
I2. a thin-line icon set interchangeable between products
I3. an icon in a tinted circle or square chip
I4. one large icon centered above every heading
I5. an icon for every bullet
I6. initials in a colored circle standing in for a person
I7. faceless 3D figures holding orbs
I8. stock "team at a laptop" or a grey placeholder box
I9. illustration style unrelated to the product
I10. an emoji favicon

## Table and list violations

R1. every cell the same weight, nothing scannable
R2. numbers left-aligned or not aligned on the decimal
R3. zebra stripes, borders and row shadows together
R4. a badge or pill in every status column
R5. an avatar in every row
R6. truncation with an ellipsis where the value is the point
R7. a row of action icons on every line

## State violations

S1. a spinner for every wait
S2. a skeleton whose shape does not match what loads
S3. an empty state made of a large icon and a centered line
S4. a toast for every outcome
S5. an error shown only as red text under a field
S6. a modal for a one-field question

## Motion violations

M1. fade-and-slide-up on every section as it scrolls in
M2. lift, scale or bounce on hover for every card
M3. a looping gradient or glow animation
M4. decorative motion nobody triggered
M5. the same duration and easing everywhere
M6. a counter animating up to a number

## Responsiveness violations

V1. a fixed desktop width
V2. everything stacking to one column with no other reflow
V3. a card grid collapsing into a tall stack of identical cards
V4. touch targets sized for a cursor

## Diagram and SVG violations

G1. rounded rectangles with drop shadows joined by arrows, the Mermaid look
G2. labels overflowing or clipped by their box
G3. arrows crossing through nodes or ending nowhere
G4. coordinates off any grid, gaps uneven
G5. a different fill per node
G6. every node the same box regardless of what it is
G7. font and colors unrelated to the page around it
G8. a legend explaining colors that carry no meaning
G9. arbitrary canvas size, content floating in it
G10. inconsistent stroke widths and arrowheads
G11. a gradient or blur filter inside the SVG
G12. a box for every noun, an arrow for every verb

## Chart and infographic violations

H1. a rainbow or categorical palette on ordered data
H2. every bar a different color
H3. 3D pie or bars
H4. pie for parts that are not a whole
H5. a donut with the total in the hole
H6. dual y-axes
H7. a y-axis not starting at zero for a bar chart
H8. smoothed curves through discrete points
H9. gridlines, legend, border and title all present because the library adds them
H10. a legend repeating what the axis already says
H11. axis labels rotated 45 degrees
H12. a title restating the axes
H13. KPI tiles with round invented numbers
H14. a sparkline in every tile
H15. an icon in a circle beside every stat
H16. a giant number over a tiny label
H17. hexagon, circle or chevron process templates
H18. a vertical timeline alternating left and right
H19. a funnel, a pyramid, three pillars
H20. more charts per view than questions asked
H21. a chart where a sentence would do

## Slide violations

Z1. one template stamped on every slide
Z2. three bullets per slide regardless of content
Z3. title, icon, bullets, no assertion made
Z4. sub-bullets
Z5. a decorative icon per bullet
Z6. a stock photo filling half of every slide
Z7. a 50/50 image and text split on every slide
Z8. a full-bleed gradient section divider
Z9. the logo on every slide
Z10. an agenda and a summary slide because decks have them
Z11. a "thank you" or "questions?" slide
Z12. a gradient title bar
