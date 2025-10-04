default:
    echo 'Hello, world!'



push :
  git add .
  git commit -m "update"
  git push repo main



clear:
   git rm --cached -r .