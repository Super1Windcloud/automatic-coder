default:
    echo 'Hello, world!'



push :
  git pull repo master
  git add .
  git commit -m "update"
  git push repo  master
  git push repo master:main

commit:
  git add . && git commit -m "update"

clear:
   git rm --cached -r .



clean :
   cd  src-tauri && cargo clean 


zip : 
   git archive -o  interview.zip HEAD
