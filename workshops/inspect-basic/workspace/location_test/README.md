## Location Extraction (Custom Evaluation)

### Dataset

**Source:** Custom CSV file: `aisi_location_extraction.csv`

**Description:** A specialized evaluation testing models' ability to extract geographic locations from conversational contexts. The dataset contains realistic dialogue snippets where locations are mentioned in various formats (coordinates, place names, landmarks) and models must extract the precise location information.

**Location Formats:**
- **Decimal coordinates:** "51.50070, -0.12460"
- **Landmarks/Places:** "Big Ben", "Edinburgh Castle", "Stonehenge"
- **Cities:** "Oxford, UK", "Newcastle upon Tyne"

**Conversation Patterns:**
- Direct coordinate statements
- Waypoint specifications in DMS (degrees, minutes, seconds)
- Landmark references in natural dialogue
- Location changes in multi-turn conversations
- Locations mentioned with distractors (prices, codes, dates)

**Size:** 87 conversation samples

**Sample:**
```
Input: "A: Meet where we watched the New Year's fireworks—Big Ben by Westminster.\nB: Same spot."
Target: "Big Ben"

Input: "A: Drop a pin at 50.7167°N, 1.8756°W – it's basically Bournemouth Pier."
Target: "50.71670, -1.87560"

Input: "A: The booking code is 743291. Anyway, take the train to Cardiff Castle and text me.\nB: Got it."
Target: "Cardiff Castle"
```

**Challenge Types:**
1. **Coordinate formats:** DMS → Decimal conversion needed
2. **Implicit references:** "the castle" with context
3. **Distractors:** Booking codes, prices, irrelevant numbers
4. **Multi-turn:** Location may be in earlier turn
5. **Format consistency:** Standardizing coordinate format

### Running the Evaluation

**Basic usage:**
```bash
cd location_test
inspect eval inspect.py
```

**With specific model:**
```bash
inspect eval inspect.py --model openai/gpt-4
inspect eval inspect.py --model anthropic/claude-3-5-sonnet-20241022
```

### Evaluation Logic

**1. Dataset Loading:**

```python
dataset = csv_dataset("./aisi_location_extraction.csv")
```

Expects CSV with columns:
- `input`: The conversation text
- `target`: The expected location extraction

**2. Solver Strategy:**

```python
solver = [
    chain_of_thought(),
    generate(),
    self_critique()
]
```

This is an **advanced multi-step solver pipeline**:

**a) `chain_of_thought()`:**
- Adds instruction to think step-by-step
- Encourages the model to reason about the location
- Helps identify which information is relevant

**b) `generate()`:**
- Produces the initial answer

**c) `self_critique()`:**
- Model reviews its own answer
- Checks if extraction is correct
- Refines if needed

**3. Scoring:**

```python
scorer = model_graded_fact()
```

Uses **model grading** because:
- Many equivalent formats: "Big Ben" vs "big ben" vs "Big Ben, London"
- Coordinates need fuzzy matching: "51.50070, -0.12460" vs "51.5007, -0.1246"
- Semantic equivalence: "Edinburgh Castle" vs "Castle in Edinburgh"

A grader model judges if the extracted location matches the target semantically.

**4. Metrics:**
- Accuracy: Percentage of correctly extracted locations

**Example Flow:**

```
Input to Model:
----
[Chain of thought instruction added]

A: Meet where we watched the New Year's fireworks—Big Ben by Westminster.
B: Same spot.

[Instruction to critique added]
----

Model Output (Step 1 - Generate):
----
Let me identify the location mentioned:
- "where we watched the New Year's fireworks"
- "Big Ben by Westminster"

The location being referenced is Big Ben, which is a famous landmark in 
Westminster, London.

ANSWER: Big Ben
----

Model Output (Step 2 - Self Critique):
----
Reviewing my answer:
- The conversation explicitly mentions "Big Ben by Westminster"
- This is the meeting location
- "Big Ben" is the precise location name
- No need to add "Westminster" or "London" as the target is the landmark

My extraction is correct: Big Ben
----

Scorer (Model Graded):
Grader model evaluates:
Q: "Is 'Big Ben' factually equivalent to 'Big Ben'?"
A: Yes → CORRECT ✓
```

**Example with Coordinates:**

```
Input:
----
A: Drop a pin at 50.7167°N, 1.8756°W – it's basically Bournemouth Pier.
----

Model Output (Chain of Thought):
----
Let me extract the location:
1. There's a coordinate: 50.7167°N, 1.8756°W
2. It's described as "basically Bournemouth Pier"
3. The question asks for the location

The coordinate is the precise location. Converting to standard format:
- Latitude: 50.7167°N = 50.71670
- Longitude: 1.8756°W = -1.87560

ANSWER: 50.71670, -1.87560
----

Model Self-Critique:
----
Checking my answer:
- Extracted coordinates correctly
- Converted to decimal format
- Used proper sign convention (W = negative)
- Formatted with 5 decimal places

This matches the standard coordinate format.
----

Scorer (Model Graded):
Q: "Is '50.71670, -1.87560' equivalent to '50.71670, -1.87560'?"
A: Yes → CORRECT ✓
```

**Example with Distractor:**

```
Input:
----
A: The booking code is 743291. Anyway, take the train to Cardiff Castle and text me.
B: Got it.
----

Model Output (Chain of Thought):
----
Analyzing the conversation:
1. "booking code is 743291" - This is a code, not a location
2. "take the train to Cardiff Castle" - This is the destination
3. "and text me" - Follow-up instruction

The location mentioned is Cardiff Castle. The booking code 743291 is 
irrelevant to the location question.

ANSWER: Cardiff Castle
----

Model Self-Critique:
----
Review:
- Correctly ignored the distractor (booking code)
- Identified the actual location (Cardiff Castle)
- Did not confuse the number with coordinates
- Provided the landmark name as requested

Answer is correct.
----

Scorer: Model judges "Cardiff Castle" ≈ "Cardiff Castle" → CORRECT ✓
```

**Example with DMS Conversion:**

```
Input:
----
A: The waypoint's in DMS: 55°14'27"N 6°30'42"W.
B: Copy that.
----

Model Output (with chain of thought):
----
The location is given in DMS (Degrees, Minutes, Seconds) format.
Converting to decimal:

Latitude: 55°14'27"N
= 55 + (14/60) + (27/3600)
= 55 + 0.2333 + 0.0075
= 55.24080°N

Longitude: 6°30'42"W  
= 6 + (30/60) + (42/3600)
= 6 + 0.5000 + 0.0117
= 6.51160°W
= -6.51160 (West is negative)

ANSWER: 55.24080, -6.51160
----

Scorer: Model judges equivalence → CORRECT ✓
```

**Key Features:**
- **Multi-format handling:** Coordinates, landmarks, cities
- **Advanced solver pipeline:** Chain of thought + self-critique
- **Model-graded scoring:** Handles format variations
- **Real-world scenarios:** Conversational context
- **Distractor resistance:** Ignores irrelevant numbers/text
- **Format normalization:** Standardizes coordinate format

**Expected Performance:**
- GPT-4: ~85-95% (excellent at format conversion)
- Claude Opus: ~80-90% (good at reasoning through context)
- Good models: 70-85%
- Weaker models: 40-60% (struggle with coordinate conversion)

**Common Failure Modes:**
1. **Coordinate conversion errors:** DMS → Decimal math mistakes
2. **Distractor confusion:** Thinking booking codes are coordinates
3. **Format inconsistency:** "51.5" vs "51.50000"
4. **Over-specification:** Adding unnecessary location details
5. **Under-specification:** "The castle" without identifying which one

**Why This Evaluation Is Useful:**
- Tests information extraction from conversational text
- Evaluates numerical reasoning (coordinate conversion)
- Assesses distractor filtering
- Measures format normalization capability
- Relevant for navigation, meeting coordination, travel applications
