# Supernova Website Documentation

This directory contains the comprehensive documentation for the Supernova blockchain website. The documentation follows a modular structure aligned with the requirements outlined in the Supernova Website Documentation Requirements document.

## Documentation Structure

Documentation is organized into the following main categories:

1. **Technical Documentation**
   - Core Architecture
   - Cryptographic Implementation
   - Security Framework

2. **Developer Resources**
   - API Documentation
   - SDK and Client Libraries
   - Smart Contract Development

3. **Node Operation**
   - Installation and Setup
   - Node Management
   - Validator Operations

4. **Environmental Impact Documentation**
   - Energy Efficiency
   - Sustainability Features

5. **Integration Guides**
   - Exchange Integration
   - Wallet Development
   - Oracle Services

6. **Governance Documentation**
   - Protocol Governance
   - Community Governance

## Hosting Strategy

The documentation will be hosted using a combination of approaches:

1. **GitHub Pages** - Primary hosting platform integrated with the repository
   - Advantages: Version control, PR workflow, automatic deployment
   - URLs: `https://supernova.dev/docs`

2. **Documentation Portal** - Interactive documentation using Docusaurus
   - Features: Search functionality, versioning, responsive design
   - Interactive API explorer embedded in the docs

3. **Markdown Integration** - Direct integration with the website's Next.js framework
   - All documentation is written in Markdown format for easy integration
   - Documentation as Code approach with automated testing

## Development Workflow

1. Create or update documentation in the appropriate directory
2. Test locally using the documentation preview tool
3. Submit a PR for review
4. After approval, documentation is automatically deployed

## Local Development

To preview the documentation locally:

```bash
# Navigate to documentation directory
cd docs/website

# Install dependencies
npm install

# Start development server
npm start
```

## Style Guide

All documentation should:
- Use Markdown formatting consistently
- Include proper headings and table of contents
- Provide code examples in relevant languages
- Include diagrams for complex concepts (as SVG files)
- Follow the voice and tone guidelines in the style guide document

## Integration with Website

The documentation is designed to seamlessly integrate with the Next.js website through:
- Shared API specification files
- Consistent styling and theming
- Cross-linking between website and documentation
- Unified search functionality 