#!/bin/bash

GHOST_DATABASE=/var/www/exonum.com/blog/content/data/ghost.db
BACKUP_DIR=/etc/db-backups/blog/
LOG_FILE=/etc/db-backups/blog/backup.log
DATE=`date '+%Y/%m/%Y-%m-%d-%H:%S'`

# Make backup directory
mkdir -p $BACKUP_DIR$DATE

# Copy Ghost Database
cp $GHOST_DATABASE $BACKUP_DIR$DATE
echo "Exonum blog db has been copied - $DATE" >> $LOG_FILE