# CLI Reference

```bash
cooper new <name>                      # scaffold a new project
cooper run                             # local dev server + infra + hot reload
cooper run --all                       # monorepo: run all apps

cooper build                           # production build
cooper deploy --env <env> --cloud <c>  # provision + deploy
cooper deploy --dry-run                # diff + cost estimate
cooper destroy --env <env>             # tear down an environment

cooper gen client --lang <lang>        # generate typed client (ts|python|rust)
cooper gen openapi                     # OpenAPI 3.1 spec
cooper gen postman                     # Postman collection

cooper db migrate                      # run pending migrations
cooper db seed                         # run seed scripts
cooper db shell                        # psql / mysql shell
cooper db conn-uri <db> --env <env>    # print connection string

cooper secrets set <name> --env <env>  # set a secret
cooper secrets ls --env <env>          # list secrets
cooper secrets rm <name> --env <env>   # remove a secret

cooper logs --env <env>                # tail logs
cooper trace --env <env>               # open trace explorer

cooper env ls                          # list environments
cooper env url <env>                   # get environment URL

cooper docs                            # serve docs locally
cooper mcp                             # start MCP server
```
