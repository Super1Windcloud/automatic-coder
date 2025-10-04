default:
    echo 'Hello, world!'



push :
  git add .
  git commit -m "update"
  git push repo  master



clear:
   git rm --cached -r .