# Authentication
## Root user authentication
JJS has builtin root user, with username "$root" (Note that $ prefix can't be used in non-special user accounts).
### Development root user
When JJS is running in development mode, you can authenticate as root, using "dev_root" string as access token
This option is automatically disabled in non-development environment, and shouldn't be turned on forcefully.
### Local Root login server in Linux
This server is started automatically by frontend and is bound to `/tmp/jjs-auth-sock`
You should connect to this socket from a process, uid of which exactly matches uid of frontend.
Local auth server will write string of form `===S===`, where S is your token