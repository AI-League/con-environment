# Models available:

ollama/deepseek-r1
ollama/qwen3-vl
ollama/llama3.1


# Introduction to AI Evaluation with Inspect

Welcome to this hands-on workshop on AI evaluation using Inspect! In this workshop, you'll learn how to systematically assess the capabilities and limitations of large language models across a diverse range of tasks.

## What is Inspect?

Inspect is a framework for large language model evaluations created by the UK AI Safety Institute. It provides a standardized way to run benchmarks, test model capabilities, and understand model behavior across different domains. Whether you're evaluating reasoning, mathematical problem-solving, or commonsense understanding, Inspect gives you the tools to do it rigorously and reproducibly.

## What You'll Learn

Throughout this workshop, you'll work with several established AI benchmarks that test different aspects of model capability:

### Mathematical Reasoning
- **GSM8K**: Grade school math word problems that test multi-step arithmetic reasoning
- **MATH Dataset**: Competition-level mathematics problems spanning algebra, geometry, number theory, and more
- **AIME 2024**: Problems from the American Invitational Mathematics Examination, testing advanced mathematical problem-solving

### Reading Comprehension & Reasoning
- **DROP**: Discrete reasoning over paragraphs, requiring models to perform calculations and logical operations on text
- **ARC (AI2 Reasoning Challenge)**: Grade-school science questions testing scientific reasoning and knowledge
- **HellaSwag**: Commonsense natural language inference about physical situations

### Specialized Tasks
- **Location Extraction**: Custom evaluation testing models' ability to parse and extract geographic information from conversational contexts (used in drones)

## Workshop Structure

Each evaluation in this workshop follows the Inspect framework's core structure:

1. **Dataset**: The questions or prompts to evaluate
2. **Solver**: How to process inputs and generate responses (e.g., chain-of-thought, few-shot prompting)
3. **Scorer**: How to determine if responses are correct
4. **Metrics**: How to aggregate results (accuracy, F1, etc.)

You'll learn how to:
- Run evaluations using the `inspect eval` command
- Customize solvers with different prompting strategies
- Implement and understand different scoring methods
- Interpret evaluation results and metrics
- Create your own custom evaluations

## Why This Matters

Understanding model capabilities through systematic evaluation is crucial for:
- **Safety**: Knowing what models can and cannot do reliably
- **Development**: Tracking improvements and regressions during model development
- **Deployment**: Making informed decisions about where models can be safely deployed
- **Research**: Contributing to the broader understanding of AI capabilities

## Getting Started

In the following sections, you'll:
1. Set up your environment and run your first evaluation
2. Explore different evaluation types hands-on
3. Learn to interpret results and debugging failed evaluations
4. Customize evaluations for your specific needs
5. Create your own evaluation from scratch

Let's dive in and start evaluating!