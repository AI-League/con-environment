# Intent
This evaluation is intended to test reasoning ability. The ARC dataset is a dataset of 7,787 genuine grade-school level, multiple-choice science questions. For each entry, the model receives as input a question and a set of multiple choice answers. Then, the model is tasked with picking the correct answer.

Here is an example of a question:
    Answer the following multiple choice question. The entire content of your response should be of the following format: 'ANSWER: $LETTER (without quotes) where LETTER is one of A,B,C,D.

    An astronomer observes that a planet rotates faster after a meteorite impact. Which is the most likely effect of this increase in rotation?

    A. Planetary density will decrease.
    B. Planetary years will become longer.
    C. Planetary days will become shorter.
    D. Planetary gravity will become stronger.

This evaluation and card was adapted from the work of Peter Clark, Isaac Cowhey, Oren Etzioni, Tushar Khot, Ashish Sabharwal, Carissa Schoenick, Oyvind Tafjord
https://arxiv.org/abs/1803.05457, and the corresponding [README](https://github.com/UKGovernmentBEIS/inspect_evals/blob/main/src/inspect_evals/arc/README.md) for the benchmark.

# Scope
This evaluation is designed to be used to assess the reasoning ability of generative models.

This evaluation should not be used for:
- 
- 
