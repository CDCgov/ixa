# Ixa Debugger Commands

The Ixa Debugger is a command line interface for pausing execution of a
simulation, reading information about its current state, and possibly manipulating
it. This document proposes a high-level grammar and initial set of commands
we should make available in the REPL, as well as a brief description of the
implementation of commands.

## General structure

Commands in the Ixa Debugger follow a hierarchical format composed
of a primary command, subcommands, and arguments:

```
<primary_command> [<subcommand>] [--<argument>[=][<value>] [<positional_argument>]]
```

Where:
- `<primary_command>` is the top-level command category (e.g., `breakpoint`, `global`, `query`, `people`).
- `[<subcommand>]` specifies an action to perform (e.g., `set`, `get`, `list`, `delete`).
- `[<positional_argument>]` may be used to provide necessary inputs (e.g., the name of a property)
- `[--<argument>[=][<value>]]` represents named options that modify behavior, or provide additional inputs

Individual commands define which components are optional or required; for example, a people
command might require an `--id` argument but optionally take a set of properties.

## Command Categories

In addition to some control flow commands (`next`, `continue`, `help`), we
might consider the following commands:

### 1. Breakpoints
Commands related to setting, listing, and removing execution breakpoints.
Note that this will require storing some internal state.

- **`breakpoint set <t>]`**

  Set a breakpoint a given time

  Example: `breakpoint set 4.0`

- **`breakpoint list`**

  List all active breakpoints.

- **`breakpoint delete <id> [--all]`**

  Delete the breakpoint with the specified id.
  Providing the `--all` option removes all breakpoints.

  Example: `breakpoint delete 1`

### 2. Globals
Commands for managing global properties in the simulation.

- **`global get <name>`**

  Retrieve the value of the specified global property by full-qualified name.

  Example: `global get ixa.Foo`

- **`global set <name>=<value>`**

  Set the specified global variable to a value.

  Example: `global set ixa.Foo=42`

- **`global unset <name>`**

  Remove the value for a specified global property.

  Example: `global unset Foo`

- **`global list`**

  List all global properties.

- **`global list --prefix <prefix>`**

  List all global variables with the specified prefix.

  Example: `global list --prefix ixa.`


### 3. People
Commands for retrieving and modifying information about entities in the simulation.

- **`people get --id <person_id> [--property <property>]`**

  Retrieve all known properties of the specified person.
  If the `--property` option is included, only the value that property will be returned.

  Example: `people get --id 42`

- **`people get --properties <property_list>`**

  Retrieve people matching the specified property list

  Example: `people get --properties Region=CA,RiskCategory=High`

- **`people get --query <query_id>`**

  Retrieve people based on the id of a previously saved query (see below).

  Example: `people get --query 1`

- **`people set --id <person_id> --property <property_name>=<value>`**

  Set the specified property of a person.

  Example: `people set --id 42 --property Region=CA`

- **`people add <k=v>`**

  Add a new person to the simulation with specified properties if required.

  Example: `people add Age=12,Risk=High`

- **`people query set <p>`**

  Define a saved query in the debugger with the specified property/value pairs.

  Example:
  ```
  query set Region=CA,RiskCategory=High
  > Created people query: 1
  ```

- **`people query get <id>`**

  Print the property/value pairs for a pre-saved query

  Example:

  ```
  query get 1
  > Region=CA,Risk=High
  ```

- **`people query list`**

  List all saved queries.


- **`people query delete <id>`**

  Delete the query with the specified id.

  Example: `query delete 1`

## Implementation

Commands are implemented as structs that are dynamically registered to a `clap`
program (the `DebuggerRepl`) given some name (e.g., `"global"`).

Given the following structure:

```
<primary_command> [<subcommand>] [--<argument>[=][<value>]] [<positional_argument>]
```

Each primary command defines the following (via the `DebuggerCommand` trait):

- Some help text to display when the `help` command is called
- A method to extend it with subcommands and arguments
- A handler that receives a reference to subcommands/arguments and the current
  `Context` for the simulation, and must return whether (1) the command exits
  the debugger, and (2) some output to display.

As an example:

```rust
struct GlobalPropertyCommand;
impl DebuggerCommand for GlobalPropertyCommand {
    fn about(&self) -> &'static str {
        "Get the value for a global property"
    }
    fn extend(&self, subcommand: Command) -> Command {
        subcommand
            .subcommand_required(true)
            .subcommand(
              Command::new("list").about("List all global properties"))
            .subcommand(
                Command::new("get")
                    .about("Get the value of a global property")
                    .arg(
                        Arg::new("property")
                            .help("The name of the global property")
                            .value_parser(value_parser!(String))
                            .required(true),
                    ),
            )
    }
    fn handle(
        &self,
        context: &mut Context,
        matches: &ArgMatches,
    ) -> Result<(bool, Option<String>), String> {
        match matches.subcommand() {
            Some(("list", _)) => Ok((false, Some(available_properties_str(context)))),
            Some(("get", m)) => {
                let name = m.get_one::<String>("property").unwrap();
                let output = context.get_serialized_value_by_string(name);
                if output.is_err() {
                    return Ok((false, output.err().map(|e| e.to_string())));
                }
                match output.unwrap() {
                    Some(value) => Ok((false, Some(value))),
                    None => Ok((false, Some(format!("Property {name} is not set")))),
                }
            }
            // This is required by the compiler will never get hit because
            // .subcommand_required(true) is set in extend
            _ => unimplemented!("subcommand required"),
        }
    }
}
```
