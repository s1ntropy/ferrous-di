# Code of Conduct

## Our Pledge

We as members, contributors, and leaders pledge to make participation in our community a harassment-free experience for everyone, regardless of age, body size, visible or invisible disability, ethnicity, sex characteristics, gender identity and expression, level of experience, education, socio-economic status, nationality, personal appearance, race, caste, color, religion, or sexual identity and orientation.

We pledge to act and interact in ways that contribute to an open, welcoming, diverse, inclusive, and healthy community.

## Our Standards

Examples of behavior that contributes to a positive environment for our community include:

### Positive Behaviors
- **Being respectful** of differing opinions, viewpoints, and experiences
- **Giving and gracefully accepting constructive feedback**
- **Accepting responsibility** and apologizing to those affected by our mistakes, and learning from the experience
- **Focusing on what is best** not just for us as individuals, but for the overall community
- **Showing empathy** towards other community members
- **Using welcoming and inclusive language**
- **Being collaborative** and helping others learn and grow

### Encouraged Contributions
- **Helping newcomers** get started with Ferrous DI
- **Sharing knowledge** through documentation, examples, and tutorials  
- **Providing constructive code review** that helps improve quality
- **Reporting bugs** with clear reproduction steps
- **Suggesting improvements** with thoughtful analysis
- **Mentoring** other contributors

Examples of unacceptable behavior include:

### Unacceptable Behaviors
- The use of sexualized language or imagery, and sexual attention or advances of any kind
- Trolling, insulting or derogatory comments, and personal or political attacks
- Public or private harassment
- Publishing others' private information, such as a physical or email address, without their explicit permission
- Conduct which could reasonably be considered inappropriate in a professional setting
- **Dismissive behavior** towards questions or contributions from newcomers
- **Bad faith arguments** or deliberately derailing discussions
- **Spam** or off-topic posts in issues and discussions

## Enforcement Responsibilities

Community leaders are responsible for clarifying and enforcing our standards of acceptable behavior and will take appropriate and fair corrective action in response to any behavior that they deem inappropriate, threatening, offensive, or harmful.

Community leaders have the right and responsibility to remove, edit, or reject comments, commits, code, wiki edits, issues, and other contributions that are not aligned to this Code of Conduct, and will communicate reasons for moderation decisions when appropriate.

## Scope

This Code of Conduct applies within all community spaces, and also applies when an individual is officially representing the community in public spaces. Examples of representing our community include using an official e-mail address, posting via an official social media account, or acting as an appointed representative at an online or offline event.

### Community Spaces Include
- GitHub repository (issues, discussions, PRs, wiki)
- Official communication channels (Discord, Matrix, etc.)
- Community events and meetups
- Social media accounts representing the project
- Any other forums created by the project team

## Reporting Guidelines

### How to Report

If you are subject to or witness unacceptable behavior, or have any other concerns, please notify the community leaders as soon as possible:

**Primary contact**: conduct@s1ntropy.dev (replace with actual email)

**Alternative contacts**:
- Create a private issue mentioning @maintainer-team
- Direct message project maintainers on GitHub

### What to Include

When reporting, please include:

- **Your contact information** (so we can follow up)
- **Names of any individuals involved** (real names or usernames)
- **Description of the incident** including what happened
- **Where it occurred** (GitHub issue, Discord, email, etc.)
- **When it occurred** (date and time if possible)
- **Any additional context** that may be helpful

### Confidentiality

All reports will be handled with discretion. We will respect confidentiality requests for the purpose of protecting victims of abuse.

## Enforcement Guidelines

Community leaders will follow these Community Impact Guidelines in determining the consequences for any action they deem in violation of this Code of Conduct:

### 1. Correction

**Community Impact**: Use of inappropriate language or other behavior deemed unprofessional or unwelcome in the community.

**Consequence**: A private, written warning from community leaders, providing clarity around the nature of the violation and an explanation of why the behavior was inappropriate. A public apology may be requested.

**Example**: Using dismissive language when responding to a newcomer's question.

### 2. Warning

**Community Impact**: A violation through a single incident or series of actions.

**Consequence**: A warning with consequences for continued behavior. No interaction with the people involved, including unsolicited interaction with those enforcing the Code of Conduct, for a specified period of time. This includes avoiding interactions in community spaces as well as external channels like social media. Violating these terms may lead to a temporary or permanent ban.

**Example**: Repeatedly making off-topic posts or ignoring requests to move discussion to appropriate channels.

### 3. Temporary Ban

**Community Impact**: A serious violation of community standards, including sustained inappropriate behavior.

**Consequence**: A temporary ban from any sort of interaction or public communication with the community for a specified period of time. No public or private interaction with the people involved, including unsolicited interaction with those enforcing the Code of Conduct, is allowed during this period. Violating these terms may lead to a permanent ban.

**Example**: Personal attacks, doxxing, or harassment of community members.

### 4. Permanent Ban

**Community Impact**: Demonstrating a pattern of violation of community standards, including sustained inappropriate behavior, harassment of an individual, or aggression toward or disparagement of classes of individuals.

**Consequence**: A permanent ban from any sort of public interaction within the community.

**Example**: Sustained harassment, threats of violence, or discriminatory behavior.

## Appeals Process

If you believe you have been unfairly penalized under this Code of Conduct, you may appeal the decision by:

1. **Contacting the appeals committee** at appeals@s1ntropy.dev
2. **Providing your case** including why you believe the decision was unfair
3. **Waiting for review** - appeals will be reviewed within 7 days
4. **Receiving a response** with the final decision

Appeals will be reviewed by community leaders who were not involved in the original decision when possible.

## Community Guidelines

### For Contributors

- **Be patient** with newcomers and those learning
- **Provide constructive feedback** in code reviews
- **Explain your reasoning** when disagreeing with proposals
- **Stay on topic** in issues and discussions
- **Use clear, professional language** in all communications

### For Maintainers  

- **Respond promptly** to reports of Code of Conduct violations
- **Be fair and consistent** in enforcement decisions
- **Document decisions** and reasoning when appropriate
- **Lead by example** in all community interactions
- **Support contributor growth** and learning

### For All Community Members

- **Read the documentation** before asking questions
- **Search existing issues** before creating new ones
- **Use appropriate channels** for different types of discussions
- **Be respectful of others' time** and expertise
- **Give back** to the community when you can

## Positive Examples

### Good Issue Reports
```
Title: Circular dependency detection fails with named services

Description:
I'm experiencing an issue where circular dependency detection doesn't work 
properly when using named services. Here's a minimal reproduction case:

[Code example]

Expected: DiError::Circular should be returned
Actual: Stack overflow occurs

Environment:
- Ferrous DI 0.1.0  
- Rust 1.75.0
- Ubuntu 22.04

I've searched existing issues and didn't find anything similar.
```

### Good Code Review Comments
```
This looks great overall! A few suggestions:

1. Consider adding error handling for the database connection (line 45)
2. The performance looks good, but we might want to add a benchmark for this path
3. Could you add a doc comment explaining the retry logic?

Nice work on the comprehensive tests! 🎉
```

### Good Community Interaction
```
@newcomer Welcome to the project! 

For your question about service lifetimes, you might find the documentation 
at [link] helpful. If you're still stuck after reading that, feel free to 
ask more specific questions.

Also, there's a similar discussion in #123 that might be relevant.
```

## Resources

### Learning Resources
- [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct)
- [Contributor Covenant](https://www.contributor-covenant.org/)
- [Open Source Guide on Building Communities](https://opensource.guide/building-community/)

### Support Resources
- **Technical questions**: GitHub Issues and Discussions
- **Code of Conduct concerns**: conduct@s1ntropy.dev
- **Security issues**: security@s1ntropy.dev
- **General inquiries**: hello@s1ntropy.dev

## Attribution

This Code of Conduct is adapted from the [Contributor Covenant](https://www.contributor-covenant.org/), version 2.1, available at https://www.contributor-covenant.org/version/2/1/code_of_conduct.html.

Community Impact Guidelines were inspired by [Mozilla's code of conduct enforcement ladder](https://github.com/mozilla/diversity).

For answers to common questions about this code of conduct, see the FAQ at https://www.contributor-covenant.org/faq. Translations are available at https://www.contributor-covenant.org/translations.

---

**Remember**: A welcoming, inclusive community benefits everyone. Let's build something great together! 🦀