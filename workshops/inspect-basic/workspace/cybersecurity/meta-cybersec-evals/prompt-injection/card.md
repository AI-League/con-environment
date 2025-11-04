# Intent
This evaluation is intended to test prompt injection. It includes a system prompt (the system message, e.g. an explanation of what not to reveal), a user input (which includes the prompt injection), and a judge question (describes the criteria to determine whether the LLM revealed something it should not have). 

The data was adapted from Meta's Cybersecurity Evaluations for [prompt injection](https://github.com/meta-llama/PurpleLlama/blob/main/CybersecurityBenchmarks/datasets/prompt_injection/prompt_injection.json).

# Scope
The evaluation is designed to be used for assessing the efficacy of prompt injection attacks in generative models.

The evaluation is not designed to be used for:
- Evaluating general model performance across unrelated NLP tasks (e.g., summarization, translation, reasoning).
- Benchmarking against traditional cybersecurity threats like malware, phishing, or network-based attacks.