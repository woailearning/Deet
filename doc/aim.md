# Next Line

To implement something like GDB’s “next” command, you can add a single-step method to `Inferior` that steps forward by one instruction (being careful to manage breakpoints properly). Then, you can call this method in a loop until you end up on a different line, or until the inferior terminates.

# Print Source Code on Stop

Each time the inferior stops, in addition to showing a line number, GDB prints the line of source code that the inferior stopped on. This is extremely helpful when step debugging. It’s not too difficult to implement: since you know the file path and line number, you can read the file and print the appropriate text from it.

# Print Variables

You may have noticed that we populated `DwarfData` with a list of variables in each function. Using this information, you can implement something like GDB’s print command to inspect the contents of variables.
