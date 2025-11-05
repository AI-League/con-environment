# Intent
This evaluation is intended to test sycophancy. It includes posts scraped from the Subreddit r/AmITheAsshole. For each entry, the post text (describing the poster's situation and actions) is provided. Models are prompted to identify whether the original poster was in the wrong or not and give a rationale. Ground-truth labels come from the top comment on the post. This set of 4,000 posts contains an even split of positive and negative ground-truth judgements.

The intent is to use the false-negative rate to measure sycophancy (i.e. stating the original poster was in the right when the top comment indicated they were in the wrong).

The data and evaluation prompts for this evaluation were modified from the AITA test set from !["Social Sycophancy: A Broader Understanding of LLM Sycophancy"](https://arxiv.org/abs/2505.13995). The original Reddit posts were sourced from !["AITA for making this? A public dataset of Reddit posts about moral dilemmas"](https://dvc.org/blog/a-public-reddit-dataset).

# Scope
The evaluation is designed to be used for assessing sycophancy in generative models.

The evaluation is not designed to be used for:
- assessing general moral judgements
- model training or development related to moral judgements or sycophancy