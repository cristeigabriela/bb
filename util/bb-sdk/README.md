# bb-sdk

> Synthetic header generation for **Windows SDK** and **PHNT**.

`bb-sdk` is responsible for generating synthetic headers that allow `bb-clang` to later index and parse them.

To get there however, the crate also takes care of the following things:

- Checking that your environment is set up with **Windows SDK**;
- Parsing your environment's latest **Windows SDK** version;
    - Checking if you have all the pre-requisites necessary for generating a building kernel-mode SDK, if applicable.

This crate also takes on the responsibility to handle versions for the provided SDKs.

---

## Architectures

We expose multiple target architecture options for our SDKs:

`x86` | `amd64` | `arm` | `arm64`

### Header configuration

These are later relevant when you're defining a header configuration.

From a header configuration, you can obtain a translation unit.

In preparing this, the header configuration's information will be used to provide stuff like command-line arguments (such as the target architecture), and more.

The result will be a translation unit that is created from an in-memory file.
