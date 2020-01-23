aws ec2 describe-instances --filters "Name=image-id,Values=ami-0e3f765791acb05e8"  --region eu-west-2 --query 'Reservations[*].Instances[*].NetworkInterfaces[*].PrivateIpAddresses[*].[Association.PublicIp]' --output text > ips

ARRAY=( $( cat ips ) )
i=1
for ip in "${ARRAY[@]}"
do
  mkdir ../raw-data/$ip
  if [ $1 = "proxy" ]; then
    scp -i ~/Downloads/some-key.pem -o StrictHostKeyChecking=no -rp ec2-user@$ip:~/results/ ../raw-data/$ip
  elif [ $1 = "server" ]; then
    scp -i ~/Downloads/some-key.pem -o StrictHostKeyChecking=no -rp ec2-user@$ip:~/results-server/ ../raw-data/$ip/results-server
  fi
done

./result-$1.sh
#./re-org.sh