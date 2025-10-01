default:
    echo 'Hello, world!'



push :
  git add .
  git commit -m "update"



clear:
   git rm --cached -r .