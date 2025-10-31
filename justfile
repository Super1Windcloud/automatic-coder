set export := true

CN_API_KEY := "cn_NZ6JCmZfDfj0NZ80ta60ZqHujE7XjEH6DQojTmp7q5IUIOIJnSgelOvoEoLNrUNnIwDEuaARGIJEMT2ZCpP1Ag"
TAURI_SIGNING_PRIVATE_KEY := "dW50cnVzdGVkIGNvbW1lbnQ6IHJzaWduIGVuY3J5cHRlZCBzZWNyZXQga2V5ClJXUlRZMEl5aTNybVZyaG9VL2NxMkFCb1VYL25WcUZ2Yy9pemFWaExnTGkwR2NnenJ5Z0FBQkFBQUFBQUFBQUFBQUlBQUFBQWthK2wxM0pCcHpqazJUaWZGenV2eUxUajAvYzFoQkw2WlJrK0d0aGQ3SDcwc3FwZlV6RjZzM1JUbHdxSXZlaWY4cHJFZXRMQXNrMlV0MU9CWEpnQjh5R3hsSWx0WUJUbE03NVdqdDkvbHI5S3JTRGZ2MHUxc08rTEtybnNDaExOclBqdGhidkdnc2c9Cg=="
TAURI_SIGNING_PRIVATE_KEY_PASSWORD := "superwindcloud"

default:
    echo  $CN_API_KEY

push:
    git pull repo master
    git add .
    git commit -m "update"
    git push repo  master
    git push repo master:main

commit:
    git add . && git commit -m "update"

clear:
    git rm --cached -r .

clean:
    cd  src-tauri && cargo clean 

zip:
    git archive -o  interview.zip HEAD

bundle:
    pnpm tb

publish:
    cn release draft --channel prod 234sdfsdf 1.0.0
    cn release upload--update-platform windows-x86_64 --channel prod --file <FILE>234sdfsdf $1.0.0
    cn release publish  --channel prod 234sdfsdf 1.0.0
